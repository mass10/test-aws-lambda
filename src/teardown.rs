use std::process::Command;
use chrono::Utc;

const FUNCTION_NAME: &str = "test-parallel-invocation";
const ROLE_NAME: &str = "test-parallel-invocation-role";

/// Lambda 関数と IAM ロールを削除するエントリポイント。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    delete_function();
    delete_role();
    log("info", "teardown", "完了。");
    Ok(())
}

/// Lambda 関数を削除する。削除済みまたはエラーの場合はスキップする。
fn delete_function() {
    let status = aws(&["lambda", "delete-function", "--function-name", FUNCTION_NAME]).status();
    match status {
        Ok(s) if s.success() => log("info", "function", &format!("関数 '{FUNCTION_NAME}' を削除した。")),
        _ => log("info", "function", &format!("関数 '{FUNCTION_NAME}' はスキップ（削除済みまたはエラー）。")),
    }
}

/// アタッチされたポリシーを先に外してから IAM ロールを削除する。削除済みまたはエラーの場合はスキップする。
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

/// タイムスタンプ・レベル・スコープ付きでログを標準出力に出力する。
fn log(level: &str, scope: &str, msg: &str) {
    let ts = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    println!("{ts} [{level}] {scope}  {msg}");
}

/// 指定した引数で AWS CLI を呼び出す Command を組み立てて返す。
fn aws(args: &[&str]) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/c").arg("aws").args(args);
    cmd
}
