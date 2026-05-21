use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const FUNCTION_NAME: &str = "test-parallel-invocation";
const PARALLEL: usize = 5;

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

                let t = Instant::now();
                let status = aws(&[
                    "lambda", "invoke",
                    "--function-name", FUNCTION_NAME,
                    "--payload", &format!("fileb://{}", payload_file.display()),
                    output_file.to_str().unwrap(),
                ])
                .status()
                .unwrap();
                let elapsed = t.elapsed();

                let body = if output_file.exists() {
                    std::fs::read_to_string(&output_file).unwrap_or_default()
                } else {
                    format!("(exit: {status})")
                };

                results.lock().unwrap().push((i, body, elapsed));
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let total = start.elapsed();
    let mut results = results.lock().unwrap();
    results.sort_by_key(|(i, _, _)| *i);

    for (i, body, elapsed) in results.iter() {
        let summary = summarize(body);
        log("info", "invoke", &format!("[{i}] {summary} ({elapsed:.2?})"));
    }
    log("info", "invoke", &format!("合計経過時間: {total:.2?}"));

    Ok(())
}

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

fn log(level: &str, scope: &str, msg: &str) {
    let ts = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    println!("{ts} [{level}] {scope}  {msg}");
}

fn aws(args: &[&str]) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.env("AWS_MAX_ATTEMPTS", "1");
    cmd.arg("/c").arg("aws").args(args);
    cmd
}

fn run(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = aws(args).status()?;
    if !status.success() {
        return Err(format!("aws {args:?} failed").into());
    }
    Ok(())
}
