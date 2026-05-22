use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const FUNCTION_NAME: &str = "test-parallel-invocation";
const PARALLEL: usize = 5;

/// 同時実行数制限なし・あり（=1）の2フェーズで並列呼び出しテストを実行するエントリポイント。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    log("info", "test", "フェーズ1: 同時実行数制限なし。");
    invoke_parallel()?;

    log("info", "test", "フェーズ2: 同時実行数 = 1。");
    run(&[
        "lambda", "put-function-concurrency",
        "--function-name", FUNCTION_NAME,
        "--reserved-concurrent-executions", "1",
    ])?;

    invoke_parallel()?;

    run(&["lambda", "delete-function-concurrency", "--function-name", FUNCTION_NAME])?;
    log("info", "test", "同時実行数制限を解除した。");

    Ok(())
}

/// Lambda 関数を PARALLEL 件数だけスレッドで並列呼び出しし、各結果と合計経過時間を出力する。
fn invoke_parallel() -> Result<(), Box<dyn std::error::Error>> {
    log("info", "invoke", &format!("{PARALLEL} 件を並列リクエスト中 (sleep=3s)。"));

    let start = Instant::now();
    let results: Arc<Mutex<Vec<(usize, String, std::time::Duration)>>> = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..PARALLEL)
        .map(|i| {
            let results = Arc::clone(&results);
            std::thread::spawn(move || {
                let tmp = std::env::temp_dir();
                let payload_file = tmp.join(format!("lambda_payload_{i}.json"));
                let output_file = tmp.join(format!("lambda_out_{i}.json"));

                std::fs::write(&payload_file, format!(r#"{{"sleep": 3, "id": {i}}}"#)).unwrap();
                let _ = std::fs::remove_file(&output_file);

                let t = Instant::now();
                let ok = aws(&[
                    "lambda", "invoke",
                    "--function-name", FUNCTION_NAME,
                    "--payload", &format!("fileb://{}", payload_file.display()),
                    output_file.to_str().unwrap(),
                ])
                .status()
                .unwrap()
                .success();
                let elapsed = t.elapsed();

                let summary = if ok {
                    let body = std::fs::read_to_string(&output_file).unwrap_or_default();
                    summarize(&body)
                } else {
                    "スロットリング".to_string()
                };

                results.lock().unwrap().push((i, summary, elapsed));
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let total = start.elapsed();
    let mut results = results.lock().unwrap();
    results.sort_by_key(|(i, _, _)| *i);

    for (i, summary, elapsed) in results.iter() {
        log("info", "invoke", &format!("[{i}] {summary} ({elapsed:.2?})"));
    }
    log("info", "invoke", &format!("合計経過時間: {total:.2?}"));

    Ok(())
}

/// Lambda 呼び出しレスポンスの JSON を「成功」「スロットリング」などの簡潔な文字列に要約する。
///
/// # Arguments
///
/// * `body` - Lambda から返ったレスポンスボディの文字列
///
/// # Returns
///
/// `"成功"`、`"スロットリング"`、または先頭 80 文字のレスポンス本文。
fn summarize(body: &str) -> String {
    let body = body.trim();
    if body.contains("TooManyRequestsException") || body.contains("Rate exceeded") {
        "スロットリング".to_string()
    } else if body.contains("\"statusCode\":200") || body.contains("\"statusCode\": 200") {
        "成功".to_string()
    } else {
        body.chars().take(80).collect()
    }
}

/// タイムスタンプ・レベル・スコープ付きでログを標準出力に出力する。
///
/// # Arguments
///
/// * `level` - ログレベル（例: `"info"`, `"error"`）
/// * `scope` - 出力元を示すスコープ名（例: `"test"`, `"invoke"`）
/// * `msg` - 出力するメッセージ
fn log(level: &str, scope: &str, msg: &str) {
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    println!("{ts} [{level}] {scope}  {msg}");
}

/// 指定した引数で AWS CLI を呼び出す Command を組み立てて返す（リトライ無効）。
///
/// # Arguments
///
/// * `args` - `aws` コマンドに渡すサブコマンドおよびオプションの配列
///
/// # Returns
///
/// 実行準備済みの `Command`。
fn aws(args: &[&str]) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.env("AWS_MAX_ATTEMPTS", "1");
    cmd.arg("/c").arg("aws").args(args);
    cmd
}

/// AWS CLI コマンドを実行し、失敗時はエラーを返す。
///
/// # Arguments
///
/// * `args` - `aws` コマンドに渡すサブコマンドおよびオプションの配列
///
/// # Returns
///
/// 成功時は `Ok(())`、コマンドが非ゼロ終了した場合はエラー。
fn run(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = aws(args).status()?;
    if !status.success() {
        return Err(format!("aws {args:?} failed").into());
    }
    Ok(())
}
