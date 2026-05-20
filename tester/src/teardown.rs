use std::process::Command;

const FUNCTION_NAME: &str = "test-parallel-invocation";
const ROLE_NAME: &str = "test-parallel-invocation-role";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    delete_function();
    delete_role();
    println!("[teardown] done.");
    Ok(())
}

fn delete_function() {
    let status = aws(&["lambda", "delete-function", "--function-name", FUNCTION_NAME]).status();
    match status {
        Ok(s) if s.success() => println!("[teardown] deleted function '{FUNCTION_NAME}'"),
        _ => println!("[teardown] skip function (already gone or error)"),
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
        Ok(s) if s.success() => println!("[teardown] deleted role '{ROLE_NAME}'"),
        _ => println!("[teardown] skip role (already gone or error)"),
    }
}

fn aws(args: &[&str]) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/c").arg("aws").args(args);
    cmd
}
