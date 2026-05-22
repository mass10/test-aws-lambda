use std::io::Write;
use std::process::Command;
use chrono::Utc;

const FUNCTION_NAME: &str = "test-parallel-invocation";
const ROLE_NAME: &str = "test-parallel-invocation-role";
const PROJECT: &str = "test-parallel-invocation";

/// IAM ロールと Lambda 関数をセットアップするエントリポイント。
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let created_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let created_by = get_caller_arn()?;
    log("info", "setup", &format!("実行者: {created_by}"));

    let role_arn = ensure_role(&created_at, &created_by)?;
    log("info", "setup", &format!("IAM ロール: {role_arn}"));

    // Lambda が IAM ロールを認識するまで待機
    log("info", "setup", "IAM ロールの反映を待機中 (10s)。");
    std::thread::sleep(std::time::Duration::from_secs(10));

    ensure_function(&role_arn, &created_at, &created_by)?;
    log("info", "setup", &format!("Lambda 関数 '{FUNCTION_NAME}' の準備完了。"));

    Ok(())
}

/// AWS STS で現在の実行者の ARN を取得して返す。
fn get_caller_arn() -> Result<String, Box<dyn std::error::Error>> {
    let out = aws(&["sts", "get-caller-identity", "--query", "Arn", "--output", "text"])
        .output()?;
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

/// IAM ロールが存在しない場合は作成してポリシーをアタッチし、ロールの ARN を返す。
///
/// # 引数
///
/// * `created_at` - リソースタグ `CreatedAt` に付与する作成日時（ISO 8601 形式）
/// * `created_by` - リソースタグ `CreatedBy` に付与する実行者 ARN
fn ensure_role(created_at: &str, created_by: &str) -> Result<String, Box<dyn std::error::Error>> {
    let out = aws(&["iam", "get-role", "--role-name", ROLE_NAME, "--query", "Role.Arn", "--output", "text"])
        .output()?;

    if out.status.success() {
        log("info", "role", "既存のロールを使用する。");
        return Ok(String::from_utf8(out.stdout)?.trim().to_string());
    }

    log("info", "role", &format!("ロール '{ROLE_NAME}' を作成する。"));

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

/// Lambda 関数が存在しない場合は ZIP をビルドして関数を作成し、アクティブになるまで待機する。
///
/// # 引数
///
/// * `role_arn` - Lambda 関数に割り当てる IAM ロールの ARN
/// * `created_at` - リソースタグ `CreatedAt` に付与する作成日時（ISO 8601 形式）
/// * `created_by` - リソースタグ `CreatedBy` に付与する実行者 ARN
fn ensure_function(role_arn: &str, created_at: &str, created_by: &str) -> Result<(), Box<dyn std::error::Error>> {
    let exists = aws(&["lambda", "get-function", "--function-name", FUNCTION_NAME])
        .output()?
        .status
        .success();

    if exists {
        log("info", "function", "既存の関数を使用する。");
        return Ok(());
    }

    log("info", "function", &format!("関数 '{FUNCTION_NAME}' を作成する。"));

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

    log("info", "function", "関数がアクティブになるまで待機中。");
    run(&["lambda", "wait", "function-active", "--function-name", FUNCTION_NAME])?;

    Ok(())
}

/// handler.py を読み込んで ZIP 圧縮し、生成したファイルのパスを返す。
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

/// AWS CLI コマンドを実行し、失敗時はエラーを返す。
fn run(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = aws(args).status()?;
    if !status.success() {
        return Err(format!("aws {args:?} failed").into());
    }
    Ok(())
}
