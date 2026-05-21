use std::io::Write;
use std::process::Command;
use chrono::Utc;

const FUNCTION_NAME: &str = "test-parallel-invocation";
const ROLE_NAME: &str = "test-parallel-invocation-role";
const PROJECT: &str = "test-parallel-invocation";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let created_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let created_by = get_caller_arn()?;
    println!("[setup] CreatedBy: {created_by}");

    let role_arn = ensure_role(&created_at, &created_by)?;
    println!("[setup] IAM role: {role_arn}");

    // Lambda が IAM ロールを認識するまで待機
    std::thread::sleep(std::time::Duration::from_secs(10));

    ensure_function(&role_arn, &created_at, &created_by)?;
    println!("[setup] Lambda function '{FUNCTION_NAME}' ready.");

    Ok(())
}

fn get_caller_arn() -> Result<String, Box<dyn std::error::Error>> {
    let out = aws(&["sts", "get-caller-identity", "--query", "Arn", "--output", "text"])
        .output()?;
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn ensure_role(created_at: &str, created_by: &str) -> Result<String, Box<dyn std::error::Error>> {
    let out = aws(&["iam", "get-role", "--role-name", ROLE_NAME, "--query", "Role.Arn", "--output", "text"])
        .output()?;

    if out.status.success() {
        return Ok(String::from_utf8(out.stdout)?.trim().to_string());
    }

    let trust_policy = r#"{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"lambda.amazonaws.com"},"Action":"sts:AssumeRole"}]}"#;

    let tag_project = format!("Key=Project,Value={PROJECT}");
    let tag_at = format!("Key=CreatedAt,Value={created_at}");
    let tag_by = format!("Key=CreatedBy,Value={created_by}");

    run(&[
        "iam", "create-role",
        "--role-name", ROLE_NAME,
        "--assume-role-policy-document", trust_policy,
        "--tags", &tag_project, &tag_at, &tag_by,
    ])?;
    run(&[
        "iam", "attach-role-policy",
        "--role-name", ROLE_NAME,
        "--policy-arn", "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole",
    ])?;

    let out = aws(&["iam", "get-role", "--role-name", ROLE_NAME, "--query", "Role.Arn", "--output", "text"])
        .output()?;

    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn ensure_function(role_arn: &str, created_at: &str, created_by: &str) -> Result<(), Box<dyn std::error::Error>> {
    let exists = aws(&["lambda", "get-function", "--function-name", FUNCTION_NAME])
        .output()?
        .status
        .success();

    if exists {
        println!("[setup] function already exists, skipping.");
        return Ok(());
    }

    let zip_path = build_zip()?;
    let zip_arg = format!("fileb://{}", zip_path.display());
    let tags = format!("Project={PROJECT},CreatedAt={created_at},CreatedBy={created_by}");

    run(&[
        "lambda", "create-function",
        "--function-name", FUNCTION_NAME,
        "--runtime", "python3.12",
        "--role", role_arn,
        "--handler", "handler.handler",
        "--zip-file", &zip_arg,
        "--timeout", "30",
        "--tags", &tags,
    ])?;

    run(&["lambda", "wait", "function-active", "--function-name", FUNCTION_NAME])?;

    Ok(())
}

fn build_zip() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let handler_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("lambda/handler.py");

    let handler_bytes = std::fs::read(&handler_path)?;

    let zip_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("lambda/handler.zip");

    let file = std::fs::File::create(&zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("handler.py", zip::write::SimpleFileOptions::default())?;
    zip.write_all(&handler_bytes)?;
    zip.finish()?;

    Ok(zip_path)
}

fn aws(args: &[&str]) -> Command {
    let mut cmd = Command::new("cmd");
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
