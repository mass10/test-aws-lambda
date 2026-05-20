use std::io::Write;
use std::process::Command;

const FUNCTION_NAME: &str = "test-parallel-invocation";
const ROLE_NAME: &str = "test-parallel-invocation-role";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let role_arn = ensure_role()?;
    println!("[setup] IAM role: {role_arn}");

    // Lambda が IAM ロールを認識するまで待機
    std::thread::sleep(std::time::Duration::from_secs(10));

    ensure_function(&role_arn)?;
    println!("[setup] Lambda function '{FUNCTION_NAME}' ready.");

    Ok(())
}

fn ensure_role() -> Result<String, Box<dyn std::error::Error>> {
    let out = Command::new("aws")
        .args(["iam", "get-role", "--role-name", ROLE_NAME, "--query", "Role.Arn", "--output", "text"])
        .output()?;

    if out.status.success() {
        return Ok(String::from_utf8(out.stdout)?.trim().to_string());
    }

    let trust_policy = r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"lambda.amazonaws.com"},"Action":"sts:AssumeRole"}]}"#;

    run("aws", &["iam", "create-role", "--role-name", ROLE_NAME, "--assume-role-policy-document", trust_policy])?;
    run("aws", &[
        "iam", "attach-role-policy",
        "--role-name", ROLE_NAME,
        "--policy-arn", "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
    ])?;

    let out = Command::new("aws")
        .args(["iam", "get-role", "--role-name", ROLE_NAME, "--query", "Role.Arn", "--output", "text"])
        .output()?;

    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn ensure_function(role_arn: &str) -> Result<(), Box<dyn std::error::Error>> {
    let exists = Command::new("aws")
        .args(["lambda", "get-function", "--function-name", FUNCTION_NAME])
        .output()?
        .status
        .success();

    if exists {
        println!("[setup] function already exists, skipping.");
        return Ok(());
    }

    let zip_path = build_zip()?;
    let zip_arg = format!("fileb://{}", zip_path.display());

    run("aws", &[
        "lambda", "create-function",
        "--function-name", FUNCTION_NAME,
        "--runtime", "python3.12",
        "--role", role_arn,
        "--handler", "handler.handler",
        "--zip-file", &zip_arg,
        "--timeout", "30",
    ])?;

    run("aws", &["lambda", "wait", "function-active", "--function-name", FUNCTION_NAME])?;

    Ok(())
}

fn build_zip() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let handler_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("lambda/handler.py");

    let handler_bytes = std::fs::read(&handler_path)?;

    let zip_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("lambda/handler.zip");

    let file = std::fs::File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("handler.py", zip::write::SimpleFileOptions::default())?;
    zip.write_all(&handler_bytes)?;
    zip.finish()?;

    Ok(zip_path)
}

fn run(cmd: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = Command::new(cmd).args(args).status()?;
    if !status.success() {
        return Err(format!("{cmd} {args:?} failed").into());
    }
    Ok(())
}
