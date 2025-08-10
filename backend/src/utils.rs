// src/utils.rs
use chrono::{Utc, Duration as ChronoDuration};
use rand::Rng;
use bcrypt::{hash, verify, DEFAULT_COST};
use regex::Regex;
//use chrono_tz::Asia::Shanghai; // 用于 UTC+8


// 辅助函数：获取“昨天”的日期字符串，格式为YYYY-MM-DD，考虑 UTC+8
// pub fn get_yesterday_trade_date_utc8() -> String {
//     let now_utc8 = Utc::now().with_timezone(&Shanghai); // 当前时间（UTC+8）
//     let yesterday_utc8 = now_utc8 - ChronoDuration::days(1);
//     yesterday_utc8.format("%Y-%m-%d").to_string()
// }


pub fn is_valid_email(email: &str) -> bool {
    let re = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
    re.is_match(email)
}

pub fn is_valid_password(password: &str) -> bool {
    password.len() >= 8 && 
    password.len() <= 32 && 
    password.chars().any(|c| c.is_ascii_uppercase())
}

pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    hash(password, DEFAULT_COST)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    verify(password, hash).unwrap_or(false)
}


pub fn get_expiration_time(minutes: i64) -> String {
    let now = Utc::now();
    let expires_at = now + ChronoDuration::minutes(minutes); // 修复：使用 ChronoDuration
    expires_at.to_rfc3339()
}

// 新增：生成邀请码的函数
pub fn generate_invite_code() -> String {
    use rand::distributions::Alphanumeric;
    let s: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8) // 生成8位长的邀请码
        .map(char::from)
        .collect();
    s.to_uppercase() // 通常邀请码会是大写
}

pub fn is_valid_date(date_str: &str) -> bool {
    // 简单的日期格式验证，例如 "YYYY-MM-DD"
    // 可以根据实际需求进行更严格的验证
    date_str.len() == 10 && date_str.contains('-')
}
//生成6位验证码
pub fn generate_verification_code() -> String {
    let code: u32 = rand::thread_rng().gen_range(100000..999999);
    code.to_string()
}
// 新增：获取当前 UTC 时间字符串
pub fn get_current_utc_time_string() -> String {
    Utc::now().to_rfc3339()
}

// 新增：验证 EVM 地址格式
pub fn is_valid_evm_address(address: &str) -> bool {
    // 简单的 EVM 地址格式验证：以 "0x" 开头，后面跟着40个十六进制字符
    // 更严格的验证可能需要外部库或更复杂的逻辑
    let re = Regex::new(r"^0x[a-fA-F0-9]{40}$").unwrap();
    re.is_match(address)
}
