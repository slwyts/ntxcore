// src/auth.rs
use actix_web::{post, web, HttpResponse, Responder,HttpRequest,put};
use serde::Deserialize;
use crate::{db::Database, utils::*};
use crate::{MailConfig, JwtConfig};
use lettre::{Transport, SmtpTransport};
use lettre::transport::smtp::authentication::Credentials;
use jsonwebtoken::{encode, Header, EncodingKey, Algorithm};
use chrono::{Utc, Duration, DateTime};
use rusqlite::Error as RusqliteError;
use crate::user::get_user_id_from_token;

// 用户修改密码请求体（需要旧密码）
#[derive(Deserialize)]
pub struct UpdatePasswordWithOldRequest {
    #[serde(rename = "oldPassword")]
    pub old_password: String,
    #[serde(rename = "newPassword")]
    pub new_password: String,
}


#[derive(Deserialize)]
pub struct RegisterRequest {
    email: String,
    nickname: String,
    verification_code: String,
    password: String,
    invite_code: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
pub struct VerificationRequest {
    email: String,
}

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    email: String,
}

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    email: String,
    reset_code: String,
    new_password: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Claims {
    pub sub: i64, // 用户ID
    pub exp: usize, // 过期时间
    pub is_admin: bool, // 新增：管理员标志
}

const ADMIN_INVITE_CODE: &str = "NTXADMIN";

#[post("/register")]
pub async fn register(
    db: web::Data<Database>,
    req: web::Json<RegisterRequest>,
) -> impl Responder {
    println!("API Call: /api/auth/register received for email: {}", req.email);

    // 验证邮箱和密码格式
    if !is_valid_email(&req.email) {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "无效的邮箱格式"}));
    }
    if !is_valid_password(&req.password) {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "密码必须为8-32个字符且包含一个大写字母"}));
    }

    // 验证验证码
    match db.get_verification_code(&req.email) {
        Ok(Some((stored_code, expires_at_str))) => {
            if stored_code != req.verification_code {
                return HttpResponse::BadRequest().json(serde_json::json!({"error": "验证码无效"}));
            }
            if let Ok(expires_at) = DateTime::parse_from_rfc3339(&expires_at_str) {
                if Utc::now() > expires_at {
                    return HttpResponse::BadRequest().json(serde_json::json!({"error": "验证码已过期"}));
                }
            } else {
                return HttpResponse::InternalServerError().json(serde_json::json!({"error": "验证码处理错误"}));
            }
        },
        Ok(None) => return HttpResponse::BadRequest().json(serde_json::json!({"error": "验证码不存在或已使用"})),
        Err(_) => return HttpResponse::InternalServerError().finish(),
    }
    
    // 检查用户是否已存在
    if db.get_user_by_email(&req.email).unwrap_or(None).is_some() {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "邮箱已被注册"}));
    }

    // 处理邀请码
    let mut is_admin_register = false;
    let inviter_email: Option<String>;

    if req.invite_code == ADMIN_INVITE_CODE {
        is_admin_register = true;
        inviter_email = Some("system@ntxdao.org".to_string()); // 管理员由系统邀请
    } else if req.invite_code == "ABCDEFGH" {
        inviter_email = Some("admin@ntxdao.org".to_string());
    } else {
        match db.get_email_by_invite_code(&req.invite_code) {
            Ok(Some(email)) => inviter_email = Some(email),
            Ok(None) => return HttpResponse::BadRequest().json(serde_json::json!({"error": "邀请码无效或不存在"})),
            Err(_) => return HttpResponse::InternalServerError().finish(),
        }
    }

    // 哈希密码
    let hashed_password = match hash_password(&req.password) {
        Ok(h) => h,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // 生成用户自己的邀请码
    let user_invite_code = generate_invite_code();

    // 启动数据库事务
    let conn_mutex = db.conn.clone();
    let mut conn = conn_mutex.lock().unwrap();
    let tx = match conn.transaction() {
        Ok(t) => t,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    // 在事务中创建用户并处理邀请码
    let registration_result = (|| -> Result<(), RusqliteError> {
        let new_user_id = db.create_user(
            &req.email,
            &req.nickname,
            &hashed_password,
            &user_invite_code,
            inviter_email.as_deref(),
            is_admin_register,
            &tx
        )?;

        if is_admin_register {
            // 使用特殊邀请码
             db.use_special_invite_code(ADMIN_INVITE_CODE, new_user_id, &tx)?;
        }
        
        // 删除验证码
        db.delete_verification_code_in_tx(&tx, &req.email)?;

        tx.commit()
    })();


    match registration_result {
        Ok(_) => {
             println!("API Success: /api/auth/register - User {} registered successfully.", req.email);
             HttpResponse::Created().json(serde_json::json!({"message": "注册成功"}))
        },
        Err(e) => {
            match e {
                RusqliteError::QueryReturnedNoRows => {
                    eprintln!("API Error: /api/auth/register - Admin invite code {} does not exist.", ADMIN_INVITE_CODE);
                    HttpResponse::BadRequest().json(serde_json::json!({"error": "管理员邀请码配置错误"}))
                },
                RusqliteError::ExecuteReturnedResults => { // 我们用这个错误来表示“码已被使用”
                    eprintln!("API Error: /api/auth/register - Admin invite code {} has already been used.", ADMIN_INVITE_CODE);
                    HttpResponse::BadRequest().json(serde_json::json!({"error": "管理员邀请码已被使用"}))
                },
                _ => {
                    eprintln!("API Error: /api/auth/register - Registration transaction failed: {:?}", e);
                    HttpResponse::InternalServerError().finish()
                }
            }
        }
    }
}


#[post("/login")]
pub async fn login(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: web::Json<LoginRequest>,
) -> impl Responder {
    println!("API Call: /api/auth/login received for email: {}", req.email);

    // 获取用户
    let user = match db.get_user_by_email(&req.email) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::BadRequest().json(serde_json::json!({"error": "邮箱或密码无效"}));
        },
        Err(e) => {
            eprintln!("API Error: /api/auth/login - Database error getting user {}: {:?}", req.email, e);
            return HttpResponse::InternalServerError().finish();
        },
    };

    // 验证密码
    let (id, nickname, hashed_password, is_admin) = user;
    if !verify_password(&req.password, &hashed_password) {
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "邮箱或密码无效"}));
    }
    // 检查用户是否激活
    if !db.is_user_active(id).unwrap_or(false) {
        eprintln!("API Error: /api/auth/login - User {} is not active.", id);
        return HttpResponse::Forbidden().json(serde_json::json!({"error": "用户账户被封禁"}));
    }
    // 生成JWT
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(256))
        .expect("有效时间戳")
        .timestamp() as usize;

    let claims = Claims {
        sub: id,
        exp: expiration,
        is_admin, // 在 JWT 中包含管理员状态
    };

    let token = match encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(jwt_config.secret.as_ref()),
    ) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("API Error: /api/auth/login - Failed to encode JWT for user {}: {:?}", id, e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    println!("API Success: /api/auth/login - User {} logged in successfully. User ID: {}, Is Admin: {}", req.email, id, is_admin);
    HttpResponse::Ok().json(serde_json::json!({
        "message": "登录成功",
        "token": token,
        "userId": id,
        "nickname": nickname,
        "isAdmin": is_admin // 在响应中也返回管理员状态
    }))
}

#[post("/send_verification_code")]
pub async fn send_verification_code(
    db: web::Data<Database>,
    mail_config: web::Data<MailConfig>,
    req: web::Json<VerificationRequest>,
) -> impl Responder {
    println!("API Call: /api/auth/send_verification_code received for email: {}", req.email);

    if !is_valid_email(&req.email) {
        eprintln!("API Error: /api/auth/send_verification_code - Invalid email format for {}", req.email);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "需要有效的邮箱地址"})
        );
    }

    // 生成验证码
    let code = generate_verification_code();
    let expires_at = get_expiration_time(10); // 10分钟有效期

    // 保存验证码
    if let Err(e) = db.create_verification_code(&req.email, &code, &expires_at) {
        eprintln!("API Error: /api/auth/send_verification_code - Failed to save verification code for {}: {:?}", req.email, e);
        return HttpResponse::InternalServerError().finish();
    }

    // 发送邮件
    let from_address = format!("NexTradeDAO <{}>", mail_config.user);
    let to_address = req.email.clone();

    let email_body = format!("您的验证码是: {}，10分钟内有效。", code);
    let email_message = match lettre::Message::builder()
        .from(from_address.parse().unwrap()) // 考虑错误处理
        .to(to_address.parse().unwrap())   // 考虑错误处理
        .subject("您的验证码")
        .body(email_body)
    {
        Ok(m) => m,
        Err(e) => {
            eprintln!("API Error: /api/auth/send_verification_code - Failed to create email message for {}: {:?}", req.email, e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "邮件内容创建失败"}));
        }
    };

    let creds = Credentials::new(mail_config.user.clone(), mail_config.pass.clone());

    // 最佳实践是可能的话一次性构建邮件发送器，或者健壮地处理错误
    let mailer = match SmtpTransport::relay("smtp.gmail.com") {
        Ok(relay) => relay.credentials(creds).build(),
        Err(e) => {
            eprintln!("API Error: /api/auth/send_verification_code - Failed to create SMTP relay: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "邮件服务配置错误"}));
        }
    };

    match mailer.send(&email_message) {
        Ok(_) => {
            println!("API Success: /api/auth/send_verification_code - Verification code email sent to: {}", req.email);
            HttpResponse::Ok().json(serde_json::json!({"message": "验证码已发送"}))
        }
        Err(e) => {
            eprintln!("API Error: /api/auth/send_verification_code - Failed to send email to {}: {:?}", req.email, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "邮件发送失败"}))
        }
    }
}

#[post("/forgot_password")]
pub async fn forgot_password(
    db: web::Data<Database>,
    mail_config: web::Data<MailConfig>,
    req: web::Json<ForgotPasswordRequest>,
) -> impl Responder {
    println!("API Call: /api/auth/forgot_password received for email: {}", req.email);

    if !is_valid_email(&req.email) {
        eprintln!("API Error: /api/auth/forgot_password - Invalid email format for {}", req.email);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "需要有效的邮箱地址"})
        );
    }

    // 检查用户是否存在
    match db.get_user_by_email(&req.email) {
        Ok(None) => {
            eprintln!("API Error: /api/auth/forgot_password - Email not registered: {}", req.email);
            return HttpResponse::BadRequest().json(
                serde_json::json!({"error": "邮箱未注册"})
            );
        }
        Ok(Some(_)) => {
            println!("API Info: /api/auth/forgot_password - User found for email: {}", req.email);
        } // 用户存在，继续
        Err(e) => {
            eprintln!("API Error: /api/auth/forgot_password - Failed to check if user exists for {}: {:?}", req.email, e);
            return HttpResponse::InternalServerError().finish();
        }
    }

    // 生成重置码
    let reset_code = generate_verification_code();
    let expires_at = get_expiration_time(10); // 10分钟有效期

    // 保存重置码
    if let Err(e) = db.create_reset_code(&req.email, &reset_code, &expires_at) {
        eprintln!("API Error: /api/auth/forgot_password - Failed to save reset code for {}: {:?}", req.email, e);
        return HttpResponse::InternalServerError().finish();
    }

    // 发送重置邮件
    let from_address = format!("NexTradeDAO <{}>", mail_config.user);
    let to_address = req.email.clone();

    let email_body = format!("您的密码重置码是: {}，10分钟内有效。", reset_code);
    let email_message = match lettre::Message::builder()
        .from(from_address.parse().unwrap()) // 考虑错误处理
        .to(to_address.parse().unwrap())   // 考虑错误处理
        .subject("密码重置请求")
        .body(email_body)
    {
        Ok(m) => m,
        Err(e) => {
            eprintln!("API Error: /api/auth/forgot_password - Failed to create reset email message for {}: {:?}", req.email, e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "邮件内容创建失败"}));
        }
    };

    let creds = Credentials::new(mail_config.user.clone(), mail_config.pass.clone());

    let mailer = match SmtpTransport::relay("smtp.gmail.com") {
        Ok(relay) => relay.credentials(creds).build(),
        Err(e) => {
            eprintln!("API Error: /api/auth/forgot_password - Failed to create SMTP relay: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "邮件服务配置错误"}));
        }
    };

    match mailer.send(&email_message) {
        Ok(_) => {
            println!("API Success: /api/auth/forgot_password - Reset email sent to: {}", req.email);
            HttpResponse::Ok().json(serde_json::json!({"message": "密码重置码已发送"}))
        }
        Err(e) => {
            eprintln!("API Error: /api/auth/forgot_password - Failed to send reset email to {}: {:?}", req.email, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "邮件发送失败"}))
        }
    }
}

#[post("/reset_password")]
pub async fn reset_password(
    db: web::Data<Database>,
    req: web::Json<ResetPasswordRequest>,
) -> impl Responder {
    println!("API Call: /api/auth/reset_password received for email: {}", req.email);

    // 验证密码复杂度
    if !is_valid_password(&req.new_password) {
        eprintln!("API Error: /api/auth/reset_password - New password weak for {}", req.email);
        return HttpResponse::BadRequest().json(
            serde_json::json!({"error": "密码必须为8-32个字符且包含一个大写字母"})
        );
    }

    // 实现重置码验证逻辑
    match db.get_reset_code(&req.email) {
        Ok(Some((stored_code, expires_at_str))) => {
            if stored_code != req.reset_code {
                eprintln!("API Error: /api/auth/reset_password - Invalid reset code for {}", req.email);
                return HttpResponse::BadRequest().json(
                    serde_json::json!({"error": "重置码无效"})
                );
            }
            match DateTime::parse_from_rfc3339(&expires_at_str) {
                Ok(expires_at) => {
                    if Utc::now() > expires_at {
                        eprintln!("API Error: /api/auth/reset_password - Expired reset code for {}", req.email);
                        return HttpResponse::BadRequest().json(
                            serde_json::json!({"error": "重置码已过期"})
                        );
                    }
                }
                Err(e) => {
                    eprintln!("API Error: /api/auth/reset_password - Failed to parse reset code expiration time for {}: {:?}", req.email, e);
                    return HttpResponse::InternalServerError().json(
                        serde_json::json!({"error": "重置码处理错误"})
                    );
                }
            }
        }
        Ok(None) => {
            eprintln!("API Error: /api/auth/reset_password - No reset code found for {}", req.email);
            return HttpResponse::BadRequest().json(
                serde_json::json!({"error": "重置码不存在或已使用，请重新请求"})
            );
        }
        Err(e) => {
            eprintln!("API Error: /api/auth/reset_password - Failed to get reset code for {}: {:?}", req.email, e);
            return HttpResponse::InternalServerError().finish();
        }
    }

    // 哈希新密码
    let hashed_password = match hash_password(&req.new_password) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("API Error: /api/auth/reset_password - Failed to hash new password for {}: {:?}", req.email, e);
            return HttpResponse::InternalServerError().finish();
        }
    };

    // 实现密码更新逻辑
    if let Err(e) = db.update_user_password(&req.email, &hashed_password) {
        eprintln!("API Error: /api/auth/reset_password - Failed to update password for {}: {:?}", req.email, e);
        return HttpResponse::InternalServerError().finish();
    }

    // 删除已使用的重置码
    if let Err(e) = db.delete_reset_code(&req.email) {
        eprintln!("API Warning: /api/auth/reset_password - Failed to delete reset code for email {}: {:?}", e, req.email);
        // 记录日志，因为密码重置已经成功。
    }

    println!("API Success: /api/auth/reset_password - Password reset successfully for {}", req.email);
    HttpResponse::Ok().json(serde_json::json!({"message": "密码重置成功"}))
}

// 用户知道旧密码修改新密码
#[put("/edit_password")] // 修改路径，避免与 user.rs 的 /profile/password 冲突，或者统一放在 user.rs
pub async fn update_user_password_with_old(
    db: web::Data<Database>,
    jwt_config: web::Data<JwtConfig>,
    req: HttpRequest,
    update_req: web::Json<UpdatePasswordWithOldRequest>,
) -> impl Responder {
    println!("API Call: /api/auth/edit_password - 收到用户修改密码请求。"); // 日志路径也改一下

    let user_id = match get_user_id_from_token(&req, &jwt_config) {
        Ok(id) => id,
        Err(resp) => {
            eprintln!("API Error: /api/auth/edit_password - 未授权访问。");
            return resp;
        },
    };

    let old_password_plain = &update_req.old_password;
    let new_password_plain = &update_req.new_password;

    // 1. 验证新密码是否符合复杂度要求 (可复用 utils 中的 is_valid_password)
    if !crate::utils::is_valid_password(new_password_plain) {
        eprintln!("API Error: /api/auth/edit_password - 新密码不符合复杂度要求。");
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "新密码不符合复杂度要求：至少8-32位，包含大写字母"}));
    }

    // 2. 获取用户的当前哈希密码 (使用 db.get_user_info_full 获取包含密码哈希的 UserFullInfo)
    let user_full_info = match db.get_user_info_full(user_id) { // 调用 get_user_info_full
        Ok(Some(info)) => info,
        Ok(None) => {
            eprintln!("API Error: /api/auth/edit_password - 用户 {} 不存在。", user_id);
            return HttpResponse::NotFound().json(serde_json::json!({"error": "用户不存在"}));
        },
        Err(e) => {
            eprintln!("API Error: /api/auth/edit_password - 获取用户 {} 完整信息失败: {:?}", user_id, e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "获取用户信息失败"}));
        },
    };

    // 3. 验证旧密码 (使用 user_full_info.password_hash)
    if !verify_password(old_password_plain, &user_full_info.password_hash) { // 修改这里
        eprintln!("API Error: /api/auth/edit_password - 用户 {} 旧密码不正确。", user_id);
        return HttpResponse::BadRequest().json(serde_json::json!({"error": "旧密码不正确"}));
    }

    // 4. 哈希新密码
    let hashed_new_password = match hash_password(new_password_plain) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("API Error: /api/auth/edit_password - 哈希新密码失败: {:?}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({"error": "密码加密失败"}));
        },
    };

    // 5. 更新数据库中的密码
    match db.update_user_password_by_id(user_id, &hashed_new_password) {
        Ok(_) => {
            println!("API Success: /api/auth/edit_password - 用户 {} 的密码已成功更新。", user_id);
            HttpResponse::Ok().json(serde_json::json!({"message": "密码更新成功"}))
        },
        Err(e) => {
            eprintln!("API Error: /api/auth/edit_password - 更新用户 {} 密码失败: {:?}", user_id, e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "更新密码失败"}))
        },
    }
}