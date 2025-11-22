// src/email.rs
use actix_web::{post, get, put, delete, web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use crate::db::{Database, EmailTask, EmailLog};
use crate::MailConfig;
use lettre::{Transport, SmtpTransport, Message};
use lettre::message::{MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use std::collections::HashMap;
use regex::Regex;

// ============================================================================
// 请求和响应结构体
// ============================================================================

#[derive(Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub subject: String,
    #[serde(rename = "htmlContent")]
    pub html_content: String,
}

#[derive(Deserialize)]
pub struct UpdateTemplateRequest {
    pub name: String,
    pub subject: String,
    #[serde(rename = "htmlContent")]
    pub html_content: String,
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    #[serde(rename = "templateId")]
    pub template_id: i64,
    pub variables: HashMap<String, String>,
    pub recipients: Vec<String>,
}

#[derive(Serialize)]
pub struct TaskDetailResponse {
    #[serde(flatten)]
    pub task: EmailTask,
    pub logs: Vec<EmailLog>,
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 替换模板中的占位符
fn replace_placeholders(template: &str, variables: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\{\{(\w+)\}\}").unwrap();
    re.replace_all(template, |caps: &regex::Captures| {
        let key = &caps[1];
        variables.get(key).cloned().unwrap_or_else(|| format!("{{{{{}}}}}", key))
    }).to_string()
}

/// 发送单封邮件
async fn send_single_email(
    mail_config: &MailConfig,
    recipient: &str,
    subject: &str,
    html_content: &str,
) -> Result<(), String> {
    // 创建邮件（明确设置UTF-8编码）
    let email = Message::builder()
        .from(mail_config.user.parse().map_err(|e| format!("Invalid sender email: {}", e))?)
        .to(recipient.parse().map_err(|e| format!("Invalid recipient email: {}", e))?)
        .subject(subject)
        .header(lettre::message::header::ContentType::parse("text/html; charset=utf-8").unwrap())
        .body(html_content.to_string())
        .map_err(|e| format!("Failed to build email: {}", e))?;

    // 创建SMTP传输 - Gmail需要STARTTLS
    let creds = Credentials::new(mail_config.user.clone(), mail_config.pass.clone());
    let mailer = SmtpTransport::starttls_relay(&mail_config.host)
        .map_err(|e| format!("Failed to create SMTP transport: {}", e))?
        .credentials(creds)
        .build();

    // 发送邮件
    mailer.send(&email)
        .map_err(|e| format!("Failed to send email: {}", e))?;

    Ok(())
}

/// 异步执行邮件发送任务
async fn execute_email_task(
    task_id: i64,
    db: web::Data<Database>,
    mail_config: web::Data<MailConfig>,
) {
    println!("[Email Task] Starting task execution: {}", task_id);

    // 更新任务状态为processing
    if let Err(e) = db.update_email_task_status(task_id, "processing") {
        eprintln!("[Email Task] Failed to update task status to processing: {}", e);
        return;
    }

    // 获取任务信息
    let task = match db.get_email_task_by_id(task_id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            eprintln!("[Email Task] Task not found: {}", task_id);
            return;
        }
        Err(e) => {
            eprintln!("[Email Task] Failed to get task: {}", e);
            return;
        }
    };

    // 获取模板
    let template = match db.get_email_template_by_id(task.template_id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            eprintln!("[Email Task] Template not found: {}", task.template_id);
            let _ = db.update_email_task_status(task_id, "failed");
            return;
        }
        Err(e) => {
            eprintln!("[Email Task] Failed to get template: {}", e);
            let _ = db.update_email_task_status(task_id, "failed");
            return;
        }
    };

    // 解析变量和收件人
    let variables: HashMap<String, String> = match serde_json::from_str(&task.variables) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[Email Task] Failed to parse variables: {}", e);
            let _ = db.update_email_task_status(task_id, "failed");
            return;
        }
    };

    let recipients: Vec<String> = match serde_json::from_str(&task.recipients) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[Email Task] Failed to parse recipients: {}", e);
            let _ = db.update_email_task_status(task_id, "failed");
            return;
        }
    };

    // 替换占位符
    let subject = replace_placeholders(&template.subject, &variables);
    let html_content = replace_placeholders(&template.html_content, &variables);

    // 发送邮件
    let mut success_count = 0;
    let mut failed_count = 0;

    for recipient in recipients.iter() {
        println!("[Email Task] Sending to: {}", recipient);

        match send_single_email(&mail_config, recipient, &subject, &html_content).await {
            Ok(_) => {
                success_count += 1;
                let _ = db.create_email_log(task_id, recipient, "success", None);
                println!("[Email Task] Successfully sent to: {}", recipient);
            }
            Err(e) => {
                failed_count += 1;
                let error_msg = format!("{}", e);
                let _ = db.create_email_log(task_id, recipient, "failed", Some(&error_msg));
                eprintln!("[Email Task] Failed to send to {}: {}", recipient, e);
            }
        }

        // 发送间隔，避免被封号
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // 更新任务统计
    if let Err(e) = db.update_email_task_counts(task_id, success_count, failed_count) {
        eprintln!("[Email Task] Failed to update task counts: {}", e);
    }

    // 更新任务状态
    let final_status = if failed_count == 0 { "completed" } else { "completed" };
    if let Err(e) = db.update_email_task_status(task_id, final_status) {
        eprintln!("[Email Task] Failed to update final task status: {}", e);
    }

    println!("[Email Task] Task completed: {} (success: {}, failed: {})", task_id, success_count, failed_count);
}

// ============================================================================
// 邮件模板 API
// ============================================================================

/// 创建邮件模板
#[post("/email/templates")]
pub async fn create_template(
    db: web::Data<Database>,
    req: web::Json<CreateTemplateRequest>,
) -> impl Responder {
    println!("API Call: POST /api/admin/email/templates");

    let template_id = match db.create_email_template(&req.name, &req.subject, &req.html_content) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to create email template: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to create template"
            }));
        }
    };

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "templateId": template_id
    }))
}

/// 获取所有邮件模板
#[get("/email/templates")]
pub async fn get_all_templates(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Call: GET /api/admin/email/templates");

    match db.get_all_email_templates() {
        Ok(templates) => HttpResponse::Ok().json(templates),
        Err(e) => {
            eprintln!("Failed to get email templates: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get templates"
            }))
        }
    }
}

/// 获取单个邮件模板
#[get("/email/templates/{id}")]
pub async fn get_template_by_id(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let template_id = path.into_inner();
    println!("API Call: GET /api/admin/email/templates/{}", template_id);

    match db.get_email_template_by_id(template_id) {
        Ok(Some(template)) => HttpResponse::Ok().json(template),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Template not found"
        })),
        Err(e) => {
            eprintln!("Failed to get email template: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get template"
            }))
        }
    }
}

/// 更新邮件模板
#[put("/email/templates/{id}")]
pub async fn update_template(
    db: web::Data<Database>,
    path: web::Path<i64>,
    req: web::Json<UpdateTemplateRequest>,
) -> impl Responder {
    let template_id = path.into_inner();
    println!("API Call: PUT /api/admin/email/templates/{}", template_id);

    match db.update_email_template(template_id, &req.name, &req.subject, &req.html_content) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "success": true
        })),
        Err(e) => {
            eprintln!("Failed to update email template: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to update template"
            }))
        }
    }
}

/// 删除邮件模板
#[delete("/email/templates/{id}")]
pub async fn delete_template(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let template_id = path.into_inner();
    println!("API Call: DELETE /api/admin/email/templates/{}", template_id);

    match db.delete_email_template(template_id) {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({
            "success": true
        })),
        Err(e) => {
            eprintln!("Failed to delete email template: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to delete template"
            }))
        }
    }
}

// ============================================================================
// 邮件任务 API
// ============================================================================

/// 创建邮件发送任务
#[post("/email/tasks")]
pub async fn create_task(
    db: web::Data<Database>,
    mail_config: web::Data<MailConfig>,
    req: web::Json<CreateTaskRequest>,
) -> impl Responder {
    println!("API Call: POST /api/admin/email/tasks");

    // 验证模板是否存在
    if db.get_email_template_by_id(req.template_id).unwrap_or(None).is_none() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Template not found"
        }));
    }

    // 验证收件人
    if req.recipients.is_empty() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Recipients cannot be empty"
        }));
    }

    // 序列化变量和收件人
    let variables_json = match serde_json::to_string(&req.variables) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to serialize variables: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid variables format"
            }));
        }
    };

    let recipients_json = match serde_json::to_string(&req.recipients) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to serialize recipients: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Invalid recipients format"
            }));
        }
    };

    let total_count = req.recipients.len() as i64;

    // 创建任务
    let task_id = match db.create_email_task(req.template_id, &variables_json, &recipients_json, total_count) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to create email task: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to create task"
            }));
        }
    };

    // 异步执行任务
    let db_clone = db.clone();
    let mail_config_clone = mail_config.clone();
    tokio::spawn(async move {
        execute_email_task(task_id, db_clone, mail_config_clone).await;
    });

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "taskId": task_id
    }))
}

/// 获取所有邮件任务
#[get("/email/tasks")]
pub async fn get_all_tasks(
    db: web::Data<Database>,
) -> impl Responder {
    println!("API Call: GET /api/admin/email/tasks");

    match db.get_all_email_tasks() {
        Ok(tasks) => HttpResponse::Ok().json(tasks),
        Err(e) => {
            eprintln!("Failed to get email tasks: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get tasks"
            }))
        }
    }
}

/// 获取单个邮件任务详情（包含日志）
#[get("/email/tasks/{id}")]
pub async fn get_task_by_id(
    db: web::Data<Database>,
    path: web::Path<i64>,
) -> impl Responder {
    let task_id = path.into_inner();
    println!("API Call: GET /api/admin/email/tasks/{}", task_id);

    // 获取任务
    let task = match db.get_email_task_by_id(task_id) {
        Ok(Some(t)) => t,
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "Task not found"
            }));
        }
        Err(e) => {
            eprintln!("Failed to get email task: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get task"
            }));
        }
    };

    // 获取日志
    let logs = match db.get_email_logs_by_task(task_id) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to get email logs: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to get logs"
            }));
        }
    };

    HttpResponse::Ok().json(TaskDetailResponse {
        task,
        logs,
    })
}
