use std::process::Command;
use chrono::Utc;

const FUNCTION_NAME: &str = "test-parallel-invocation";
const ROLE_NAME: &str = "test-parallel-invocation-role";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    delete_function();
    delete_role();
    log("info", "teardown", "完了。");
    Ok(())
}

fn delete_function() {
    let status = aws(&["lambda", "delete-function", "--function-name", FUNCTION_NAME]).status();
    match status {
        Ok(s) if s.success() => log("info", "function", &format!("関数 '{FUNCTION_NAME}' を削除した。")),
        _ => log("info", "function", &format!("関数 '{FUNCTION_NAME}' はスキップ（削除済みまたはエラー）。")),
    }
}

fn delete_role() {
    let _ = aws(&[
        "iam", "detach-role-policy",
        "--role-name", ROLE_NAME,
        "--policy-arn", "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
    ]).status();

    let status = aws(&["iam", "delete-role", "--role-name", ROLE_NAME]).status();
    match status {
        Ok(s) if s.success() => log("info", "role", &format!("ロール '{ROLE_NAME}' を削除した。")),
        _ => log("info", "role", &format!("ロール '{ROLE_NAME}' はスキップ（削除済みまたはエラー）。")),
    }
}

fn log(level: &str, scope: &str, msg: &str) {
    let ts = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    println!("{ts} [{level}] {scope}  {msg}");
}

fn aws(args: &[&str]) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/c").arg("aws").args(args);
    cmd
}
