use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Instant;

const FUNCTION_NAME: &str = "test-parallel-invocation";
const PARALLEL: usize = 5;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 並列呼び出し可能（制限なし）===");
    invoke_parallel()?;

    println!("\n=== 並列呼び出し不可（同時実行数 = 1）===");
    run(&[
        "lambda", "put-function-concurrency",
        "--function-name", FUNCTION_NAME,
        "--reserved-concurrent-executions", "1",
    ])?;

    invoke_parallel()?;

    run(&["lambda", "delete-function-concurrency", "--function-name", FUNCTION_NAME])?;
    println!("\n[test] reserved concurrency 解除済み");

    Ok(())
}

fn invoke_parallel() -> Result<(), Box<dyn std::error::Error>> {
    println!("  {PARALLEL} 並列リクエスト (sleep=3s) ...");

    let start = Instant::now();
    let results: Arc<Mutex<Vec<(usize, String)>>> = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..PARALLEL)
        .map(|i| {
            let results = Arc::clone(&results);
            std::thread::spawn(move || {
                let tmp = std::env::temp_dir();
                let payload_file = tmp.join(format!("lambda_payload_{i}.json"));
                let output_file = tmp.join(format!("lambda_out_{i}.json"));

                std::fs::write(&payload_file, format!(r#"{{"sleep": 3, "id": {i}}}"#)).unwrap();

                let status = aws(&[
                    "lambda", "invoke",
                    "--function-name", FUNCTION_NAME,
                    "--payload", &format!("fileb://{}", payload_file.display()),
                    output_file.to_str().unwrap(),
                ])
                .status()
                .unwrap();

                let body = if output_file.exists() {
                    std::fs::read_to_string(&output_file).unwrap_or_default()
                } else {
                    format!("(exit: {status})")
                };

                results.lock().unwrap().push((i, body));
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    let mut results = results.lock().unwrap();
    results.sort_by_key(|(i, _)| *i);

    for (i, body) in results.iter() {
        println!("  [{i}] {body}");
    }
    println!("  elapsed: {elapsed:.2?}");

    Ok(())
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
