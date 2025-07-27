// src/db.rs
use rusqlite::{Connection, Result, params, params_from_iter, OptionalExtension, Transaction, Error as RusqliteError};
use std::path::Path;
use std::sync::{Arc, Mutex};
use serde::Serialize;
use std::collections::HashMap;
use chrono::{Utc};
use rusqlite::ffi; // Import ffi for custom error creation

pub struct Database {
    pub conn: Arc<Mutex<Connection>>,
}


impl Database {
    pub fn new(db_file: &str) -> Result<Self> {
        let file_exists = Path::new(db_file).exists();
        let conn = Connection::open(db_file)?;

        if !file_exists {
            println!("数据库文件不存在，正在初始化...");
        } else {
            // 文件已存在时，也打印初始化信息，表示即将检查并创建可能的缺失表
            println!("数据库文件已存在，检查并同步数据库结构...");
        }

        // 无论文件是否存在，都执行数据库初始化逻辑
        // 因为 CREATE TABLE IF NOT EXISTS 只会在表不存在时创建
        // 所以对于已存在的表不会有影响，但会创建新增的表
        Self::initialize_database(&conn)?;
        println!("数据库结构同步完成。");

        Ok(Database {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn initialize_database(conn: &Connection) -> Result<()> {
        // 用户表 - 新增 is_admin 和 created_at 字段
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT UNIQUE NOT NULL,
                nickname TEXT NOT NULL,
                password TEXT NOT NULL,
                inviteCode TEXT NOT NULL UNIQUE,
                inviteBy TEXT,
                exp INTEGER NOT NULL DEFAULT 0,
                usdt_balance REAL NOT NULL DEFAULT 0.0,
                ntx_balance REAL NOT NULL DEFAULT 0.0,
                is_active BOOLEAN DEFAULT TRUE NOT NULL,
                is_admin BOOLEAN NOT NULL DEFAULT FALSE,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                gntx_balance REAL DEFAULT 0.0,
                is_broker BOOLEAN NOT NULL DEFAULT FALSE 
            )
            "#,
            [],
        )?;
        //is_broker标记 若为是 强制为经纪商 即使不满足条件

        // 特殊邀请码表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS special_invite_codes (
                code TEXT PRIMARY KEY NOT NULL,
                is_used BOOLEAN NOT NULL DEFAULT FALSE,
                used_by_user_id INTEGER,
                used_at TEXT
            )
            "#,
            [],
        )?;
        
        // 插入特殊的管理员邀请码
        conn.execute(
            "INSERT OR IGNORE INTO special_invite_codes (code, is_used) VALUES ('NTXADMIN', FALSE)",
            [],
        )?;

        // 验证码表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS verification_codes (id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE, code TEXT NOT NULL, expiresAt TEXT NOT NULL)",
            [],
        )?;

        // 重置码表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS reset_codes (id INTEGER PRIMARY KEY, email TEXT NOT NULL UNIQUE, code TEXT NOT NULL, expiresAt TEXT NOT NULL)",
            [],
        )?;

        // 平台数据表 
        conn.execute(
            "CREATE TABLE IF NOT EXISTS platform_data (id INTEGER PRIMARY KEY, totalMined REAL NOT NULL DEFAULT 0, totalCommission REAL NOT NULL DEFAULT 0, totalBurned REAL NOT NULL DEFAULT 0, totalTradingVolume REAL NOT NULL DEFAULT 0, platformUsers INTEGER NOT NULL DEFAULT 0, genesis_date TEXT NOT NULL DEFAULT (strftime('%Y-%m-%d', 'now', 'utc', '+8 hours')))",
            [],
        )?;

        // 用户数据表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_data (id INTEGER PRIMARY KEY, userId INTEGER NOT NULL UNIQUE, totalMining REAL NOT NULL DEFAULT 0, totalTradingCost REAL NOT NULL DEFAULT 0, FOREIGN KEY (userId) REFERENCES users(id))",
            [],
        )?;

        // 每日平台数据表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daily_platform_data (id INTEGER PRIMARY KEY, date TEXT NOT NULL UNIQUE, miningOutput REAL NOT NULL DEFAULT 0, burned REAL NOT NULL DEFAULT 0, commission REAL NOT NULL DEFAULT 0, tradingVolume REAL NOT NULL DEFAULT 0, miners INTEGER NOT NULL DEFAULT 0)",
            [],
        )?;

        // 每日用户数据表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daily_user_data (id INTEGER PRIMARY KEY, userId INTEGER NOT NULL, date TEXT NOT NULL, miningOutput REAL NOT NULL DEFAULT 0, totalTradingCost REAL NOT NULL DEFAULT 0, FOREIGN KEY (userId) REFERENCES users(id), UNIQUE(userId, date))",
            [],
        )?;

        // 交易所表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS exchanges (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, logoUrl TEXT NOT NULL, miningEfficiency REAL NOT NULL, cex_url TEXT NOT NULL)",
            [],
        )?;

        // 用户交易所绑定表
        // 关键在于这个表的 UNIQUE(userId, exchangeId) 约束
        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_exchanges (id INTEGER PRIMARY KEY, userId INTEGER NOT NULL, exchangeId INTEGER NOT NULL, exchange_uid TEXT NOT NULL, isBound BOOLEAN NOT NULL DEFAULT 0, FOREIGN KEY (userId) REFERENCES users(id), FOREIGN KEY (exchangeId) REFERENCES exchanges(id), UNIQUE(userId, exchangeId))",
            [],
        )?;

        // 挖矿排行榜表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS mining_leaderboard (id INTEGER PRIMARY KEY, date TEXT NOT NULL, userId INTEGER NOT NULL, nickname TEXT NOT NULL, miningAmount REAL NOT NULL, rank INTEGER NOT NULL, FOREIGN KEY (userId) REFERENCES users(id), UNIQUE(date, userId))",
            [],
        )?;

        // 提现订单表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS withdrawal_orders (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, user_email TEXT NOT NULL, amount REAL NOT NULL, currency TEXT NOT NULL, to_address TEXT NOT NULL, is_confirmed BOOLEAN NOT NULL DEFAULT 0, created_at TEXT NOT NULL, processed_at TEXT, status TEXT NOT NULL DEFAULT 'pending', FOREIGN KEY (user_id) REFERENCES users(id))",
            [],
        )?;

        // 每日用户交易记录表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS daily_user_trades (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, user_email TEXT NOT NULL, exchange_id INTEGER NOT NULL, exchange_name TEXT NOT NULL, trade_volume_usdt REAL NOT NULL, fee_usdt REAL NOT NULL, trade_date TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), UNIQUE(user_id, exchange_id, trade_date), FOREIGN KEY (user_id) REFERENCES users(id), FOREIGN KEY (exchange_id) REFERENCES exchanges(id))",
            [],
        )?;

        // 佣金发放记录表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS commission_records (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, invited_user_id INTEGER NOT NULL, commission_amount REAL NOT NULL, commission_currency TEXT NOT NULL, record_date TEXT NOT NULL, created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), FOREIGN KEY (user_id) REFERENCES users(id), FOREIGN KEY (invited_user_id) REFERENCES users(id))",
            [],
        )?;

        // DAO 拍卖表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS dao_auctions (id INTEGER PRIMARY KEY, admin_bsc_address TEXT NOT NULL, start_time TEXT NOT NULL, end_time TEXT NOT NULL, is_active BOOLEAN NOT NULL DEFAULT 1)",
            [],
        )?;

        // 用户 BSC 地址绑定表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_bsc_addresses (user_id INTEGER PRIMARY KEY, bsc_address TEXT NOT NULL UNIQUE, bound_at TEXT NOT NULL, FOREIGN KEY (user_id) REFERENCES users(id))",
            [],
        )?;

        // Academy 文章表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS academy_articles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                summary TEXT NOT NULL,
                image_url TEXT,
                publish_date TEXT NOT NULL,
                modify_date TEXT NOT NULL,
                is_displayed BOOLEAN NOT NULL,
                content TEXT NOT NULL
            )
            "#,
            [],
        )?;

        // NTX 分配比例控制表
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS ntx_control_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1), -- Enforce only one row
                admin_fee_percentage REAL NOT NULL DEFAULT 90.0
            )
            "#,
            [],
        )?;
        // 插入默认值
        conn.execute(
            "INSERT OR IGNORE INTO ntx_control_settings (id, admin_fee_percentage) VALUES (1, 90.0)",
            [],
        )?;

        // 插入初始平台数据 - 自动设置 genesis_date 为当前 UTC+8 日期
        conn.execute(
            "INSERT OR IGNORE INTO platform_data (id, totalMined, totalCommission, totalBurned, totalTradingVolume, platformUsers, genesis_date) VALUES (1, 0.0, 0.0, 0.0, 0.0, 0, strftime('%Y-%m-%d', 'now', 'utc', '+8 hours'))",
            [],
        )?;

        // 插入交易所数据
        let exchanges = vec![
            ("Bitget", "/bitget.png", 60.0, "https://partner.niftah.cn/bg/RM1W4H"),
            ("Htx", "/htx.jpeg", 50.0, "https://www.htx.com.ve/invite/zh-cn/1h?invite_code=dn2dc223"),
            ("BYBIT", "/bybit.png", 44.4, "https://partner.bybit.com/b/128308"),
            ("Binance", "/binance.png", 41.0, "https://www.binance.com/join?ref=169809979"),
            ("XT", "/xt.png", 70.0, "https://www.xt.com/en/accounts/register?ref=BTEH6V"),
        ];
        for (name, logo_url, mining_efficiency, cex_url) in exchanges {
            conn.execute(
                "INSERT OR IGNORE INTO exchanges (name, logoUrl, miningEfficiency, cex_url) VALUES (?1, ?2, ?3, ?4)",
                &[name, logo_url, &mining_efficiency.to_string(), cex_url],
            )?;
        }

        // 为 withdrawal_orders 表的 status 字段创建索引，加速查询
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_withdrawal_status ON withdrawal_orders (status)",
            [],
        )?;

        // 为 users 表的 created_at 字段创建索引，加速日期相关的用户查询
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_users_created_at ON users (created_at)",
            [],
        )?;

        Ok(())
    }
    
    // 新增: 检查用户是否为经纪商 (Broker)
    pub fn is_broker(&self, user_id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        // 获取 gntx_balance 和 email
        let (gntx_balance, email, is_broker_flag): (f64, String, bool) = match conn.query_row(
            "SELECT gntx_balance, email, is_broker FROM users WHERE id = ?",
            params![user_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        ) {
            Ok(data) => data,
            Err(_) => return Ok(false), // 如果用户不存在，则不是经纪商
        };
    
        // 强制经纪商
        if is_broker_flag {
            return Ok(true);
        }

        // 获取邀请的用户数量
        let invited_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE inviteBy = ?",
            params![email],
            |row| row.get(0),
        )?;
        
        // 判断是否满足经纪商条件
        Ok(gntx_balance >= 1.0 && invited_count >= 100)
    }

    // 检查用户是否为管理员
    pub fn is_user_admin(&self, user_id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT is_admin FROM users WHERE id = ?",
            params![user_id],
            |row| row.get(0),
        ).optional().map(|r| r.unwrap_or(false))
    }

    // 获取管理员仪表盘数据
    pub fn get_admin_dashboard_data(&self) -> Result<AdminDashboardData> {
        let conn = self.conn.lock().unwrap(); // 在函数开始时获取一次锁

        // 获取待处理提现订单数量
        let pending_withdrawals: i64 = conn.query_row(
            "SELECT COUNT(*) FROM withdrawal_orders WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )?;
        
        // 获取今日新增用户数量
        // 注意：这里使用 date() 函数会阻止索引的完全利用，但对于小到中等规模的数据集影响不大。
        // 对于非常大的数据集，可以考虑将 created_at 存储为 DATE 类型或使用 BETWEEN 范围查询。
        let today_date_str = Utc::now().format("%Y-%m-%d").to_string();
        let new_users_today: i64 = conn.query_row(
            "SELECT COUNT(*) FROM users WHERE date(created_at) = ?",
            params![today_date_str],
            |row| row.get(0),
        )?;

        // 获取平台总数据 - 调用内部函数，并传入已经持有的连接锁
        let platform_data = Self::_get_platform_data_internal(&conn)?;

        Ok(AdminDashboardData {
            pending_withdrawals,
            new_users_today,
            total_mined: platform_data.total_mined,
            total_commission: platform_data.total_commission,
            total_burned: platform_data.total_burned,
            total_trading_volume: platform_data.total_trading_volume,
            platform_users: platform_data.platform_users,
        })
    }
    
    // 新增: 获取多个用户的邀请数量
    pub fn get_invited_user_counts(&self, user_ids: &[i64]) -> Result<HashMap<i64, i64>> {
        if user_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let conn = self.conn.lock().unwrap();
        
        // 1. 获取这些用户的邮箱
        let params_placeholders = user_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT id, email FROM users WHERE id IN ({})", params_placeholders);
        let user_emails: HashMap<i64, String> = conn.prepare(&sql)?
            .query_map(params_from_iter(user_ids.iter()), |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<HashMap<_, _>, _>>()?;

        if user_emails.is_empty() {
            return Ok(HashMap::new());
        }

        // 2. 按邮箱查询邀请数量
        let emails: Vec<String> = user_emails.values().cloned().collect();
        let email_placeholders = emails.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let count_sql = format!(
            "SELECT inviteBy, COUNT(*) FROM users WHERE inviteBy IN ({}) GROUP BY inviteBy",
            email_placeholders
        );

        let email_counts: HashMap<String, i64> = conn.prepare(&count_sql)?
            .query_map(params_from_iter(emails.iter()), |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<HashMap<_, _>, _>>()?;

        // 3. 结果映射回 user_id
        let mut result_counts = HashMap::new();
        for (user_id, email) in user_emails {
            result_counts.insert(user_id, *email_counts.get(&email).unwrap_or(&0));
        }

        Ok(result_counts)
    }

    // 新增: 获取所有推荐关系作为 Map (被邀请人ID -> 邀请人ID)
    pub fn get_all_referral_relationships_as_map(&self) -> Result<HashMap<i64, i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT ui.id, u.id
            FROM users u
            JOIN users ui ON u.email = ui.inviteBy
            WHERE u.id IS NOT NULL AND ui.id IS NOT NULL
            "#
        )?;
        let pairs = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?.collect::<Result<Vec<(i64, i64)>, _>>()?;

        Ok(pairs.into_iter().collect())
    }


    // 在事务中处理特殊邀请码
    pub fn use_special_invite_code(&self, code: &str, user_id: i64, tx: &Transaction) -> Result<()> {
        let is_used: bool = tx.query_row(
            "SELECT is_used FROM special_invite_codes WHERE code = ?",
            params![code],
            |row| row.get(0),
        ).optional()?.ok_or_else(|| RusqliteError::QueryReturnedNoRows)?;

        if is_used {
            return Err(rusqlite::Error::ExecuteReturnedResults);
        }

        let current_time = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE special_invite_codes SET is_used = TRUE, used_by_user_id = ?, used_at = ? WHERE code = ?",
            params![user_id, current_time, code],
        )?;

        Ok(())
    }

    // 根据邮箱获取用户ID、昵称、密码和管理员状态
    pub fn get_user_by_email(&self, email: &str) -> Result<Option<(i64, String, String, bool)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, nickname, password, is_admin FROM users WHERE email = ?")?;
        stmt.query_row(params![email], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        }).optional()
    }

    // 获取用户信息 (用户端使用，不含敏感信息)
    pub fn get_user_info(&self, user_id: i64) -> Result<Option<UserInfo>> {
        let conn = self.conn.lock().unwrap();
        // 在 SELECT 查询中增加 gntx_balance 字段
        let mut stmt = conn.prepare(
            "SELECT id, nickname, email, inviteCode, inviteBy, exp, usdt_balance, ntx_balance, is_active, gntx_balance FROM users WHERE id = ?"
        )?;
        stmt.query_row(params![user_id], |row| {
            Ok(UserInfo {
                id: row.get(0)?,
                nickname: row.get(1)?,
                email: row.get(2)?,
                my_invite_code: row.get(3)?,
                invited_by: row.get(4)?,
                exp: row.get(5)?,
                usdt_balance: row.get(6)?,
                ntx_balance: row.get(7)?,
                is_active: row.get(8)?,
                gntx_balance: row.get(9)?, // 新增：映射 gntx_balance
            })
        }).optional()
    }

    // *** 新增函数：根据用户邮箱获取其邀请的用户数量 ***
    pub fn get_invited_user_count_by_email(&self, email: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM users WHERE inviteBy = ?",
            params![email],
            |row| row.get(0),
        )
    }

    //管理员获取用户完整信息
    pub fn get_user_info_full(&self, user_id: i64) -> Result<Option<UserFullInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, email, nickname, password, inviteCode, inviteBy, exp, usdt_balance, ntx_balance, is_active, is_admin, is_broker, created_at FROM users WHERE id = ?"
        )?;
        stmt.query_row(params![user_id], |row| {
            Ok(UserFullInfo {
                id: row.get(0)?,
                email: row.get(1)?,
                nickname: row.get(2)?,
                password_hash: row.get(3)?,
                my_invite_code: row.get(4)?,
                invited_by: row.get(5)?,
                exp: row.get(6)?,
                usdt_balance: row.get(7)?,
                ntx_balance: row.get(8)?,
                is_active: row.get(9)?,
                is_admin: row.get(10)?,
                is_broker: row.get(11)?,
                created_at: row.get(12)?,
            })
        }).optional()
    }
    
    // 创建用户，包含 is_admin
    pub fn create_user(&self, email: &str, nickname: &str, password: &str, invite_code: &str, invite_by: Option<&str>, is_admin: bool, tx: &Transaction) -> Result<i64> {
        tx.execute(
            "INSERT INTO users (email, nickname, password, inviteCode, inviteBy, is_admin) VALUES (?, ?, ?, ?, ?, ?)",
            params![email, nickname, password, invite_code, invite_by, is_admin],
        )?;
        Ok(tx.last_insert_rowid())
    }

    // 新增: 管理员删除用户 (谨慎操作)
    pub fn delete_user(&self, user_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        // 在实际应用中，删除用户可能需要级联删除其所有相关数据
        // 为了简化，这里只删除用户表中的记录
        conn.execute("DELETE FROM users WHERE id = ?", params![user_id])?;
        Ok(())
    }
    
    // 获取用户绑定的交易所信息
    pub fn get_user_exchanges(&self, user_id: i64) -> Result<Vec<ExchangeInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT e.id, e.name, e.logoUrl, e.miningEfficiency, e.cex_url
            FROM user_exchanges ue
            JOIN exchanges e ON ue.exchangeId = e.id
            WHERE ue.userId = ? AND ue.isBound = 1
            "#
        )?;

        let exchanges = stmt.query_map(params![user_id], |row| {
            Ok(ExchangeInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                logo_url: row.get(2)?,
                mining_efficiency: row.get(3)?,
                cex_url: row.get(4)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(exchanges)
    }

    pub fn get_user_id_by_exchange_uid(&self, exchange_id: i64, exchange_uid: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT userId FROM user_exchanges WHERE exchangeId = ?1 AND exchange_uid = ?2",
            params![exchange_id, exchange_uid],
            |row| row.get(0),
        )
        .optional()
    }
    // 根据邀请码获取邮箱
    pub fn get_email_by_invite_code(&self, invite_code: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT email FROM users WHERE inviteCode = ?")?;
        stmt.query_row(params![invite_code], |row| row.get(0)).optional()
    }

    // 根据邀请码获取用户ID
    pub fn get_user_id_by_invite_code(&self, invite_code: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT id FROM users WHERE inviteCode = ?", params![invite_code], |row| row.get(0))
            .optional()
    }
    
    // 更新用户密码
    pub fn update_user_password(&self, email: &str, new_password_hash: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let rows_affected = conn.execute(
            "UPDATE users SET password = ? WHERE email = ?",
            params![new_password_hash, email],
        )?;
        if rows_affected == 0 {
            eprintln!("没有找到邮箱为 {} 的用户来更新密码。", email);
        }
        Ok(())
    }

    // 更新用户余额 (用于提现)
    pub fn update_user_balance(&self, user_id: i64, currency: &str, amount: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let query = format!("UPDATE users SET {}_balance = {}_balance + ? WHERE id = ?", currency, currency);
        conn.execute(&query, params![amount, user_id])?;
        Ok(())
    }

    // 创建提现订单
    pub fn create_withdrawal_order(&self, user_id: i64, user_email: &str, amount: f64, currency: &str, to_address: &str, created_at: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO withdrawal_orders (user_id, user_email, amount, currency, to_address, is_confirmed, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![user_id, user_email, amount, currency, to_address, false, created_at],
        )?;
        Ok(())
    }

    // 验证码操作
    pub fn create_verification_code(&self, email: &str, code: &str, expires_at: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO verification_codes (email, code, expiresAt) VALUES (?, ?, ?)",
            params![email, code, expires_at],
        )?;
        Ok(())
    }

    // 获取验证码
    pub fn get_verification_code(&self, email: &str) -> Result<Option<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT code, expiresAt FROM verification_codes WHERE email = ? ORDER BY id DESC LIMIT 1"
        )?;
        stmt.query_row(params![email], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).optional()
    }

    // 删除验证码
    pub fn delete_verification_code(&self, email: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM verification_codes WHERE email = ?", params![email])?;
        Ok(())
    }

    // 重置码操作
    pub fn create_reset_code(&self, email: &str, code: &str, expires_at: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO reset_codes (email, code, expiresAt) VALUES (?, ?, ?)",
            params![email, code, expires_at],
        )?;
        Ok(())
    }

    // 获取重置码
    pub fn get_reset_code(&self, email: &str) -> Result<Option<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT code, expiresAt FROM reset_codes WHERE email = ?")?;
        stmt.query_row(params![email], |row| {
            Ok((row.get(0)?, row.get(1)?))
        }).optional()
    }

    // 删除重置码
    pub fn delete_reset_code(&self, email: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM reset_codes WHERE email = ?", params![email])?;
        Ok(())
    }

    // 内部辅助函数：获取平台数据，需要传入一个已锁定的 Connection 引用
    fn _get_platform_data_internal(conn: &Connection) -> Result<PlatformData> {
        let mut stmt = conn.prepare(
            "SELECT totalMined, totalCommission, totalBurned, totalTradingVolume, platformUsers, genesis_date
             FROM platform_data WHERE id = 1"
        )?;

        stmt.query_row([], |row| {
            Ok(PlatformData {
                total_mined: row.get(0)?,
                total_commission: row.get(1)?,
                total_burned: row.get(2)?,
                total_trading_volume: row.get(3)?,
                platform_users: row.get(4)?,
                genesis_date: row.get(5)?,
            })
        })
    }

    // 公共函数：获取平台总数据，会自己获取锁
    pub fn get_platform_data(&self) -> Result<PlatformData> {
        let conn = self.conn.lock().unwrap();
        Self::_get_platform_data_internal(&conn)
    }

    // 获取每日平台数据
    pub fn get_daily_platform_data(&self, date: &str) -> Result<Option<DailyPlatformData>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT miningOutput, burned, commission, tradingVolume, miners
             FROM daily_platform_data WHERE date = ?"
        )?;
        stmt.query_row(params![date], |row| {
            Ok(DailyPlatformData {
                mining_output: row.get(0)?,
                burned: row.get(1)?,
                commission: row.get(2)?,
                trading_volume: row.get(3)?,
                miners: row.get(4)?,
            })
        }).optional()
    }

    // 新增: 获取历史平台数据 (日期范围)
    pub fn get_historical_platform_data(&self, start_date: &str, end_date: &str) -> Result<Vec<HistoricalPlatformData>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT date, miningOutput, burned, commission, tradingVolume, miners FROM daily_platform_data WHERE date BETWEEN ? AND ? ORDER BY date ASC"
        )?;
        let data = stmt.query_map(params![start_date, end_date], |row| {
            Ok(HistoricalPlatformData {
                date: row.get(0)?,
                mining_output: row.get(1)?,
                burned: row.get(2)?,
                commission: row.get(3)?,
                trading_volume: row.get(4)?,
                miners: row.get(5)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(data)
    }
    
    // 获取所有交易所
    pub fn get_exchanges(&self) -> Result<Vec<ExchangeInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, logoUrl, miningEfficiency, cex_url FROM exchanges"
        )?;

        let exchanges = stmt.query_map([], |row| {
            Ok(ExchangeInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                logo_url: row.get(2)?,
                mining_efficiency: row.get(3)?,
                cex_url: row.get(4)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(exchanges)
    }

    // 创建交易所
    pub fn create_exchange(&self, name: &str, logo_url: &str, mining_efficiency: f64, cex_url: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO exchanges (name, logoUrl, miningEfficiency, cex_url) VALUES (?, ?, ?, ?)",
            params![name, logo_url, mining_efficiency, cex_url],
        )?;
        Ok(conn.last_insert_rowid())
    }

    // 更新交易所
    pub fn update_exchange(&self, id: i64, name: &str, logo_url: &str, mining_efficiency: f64, cex_url: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE exchanges SET name = ?, logoUrl = ?, miningEfficiency = ?, cex_url = ? WHERE id = ?",
            params![name, logo_url, mining_efficiency, cex_url, id],
        )?;
        Ok(())
    }

    // 删除交易所
    pub fn delete_exchange(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM exchanges WHERE id = ?", params![id])?;
        Ok(())
    }

    // 绑定用户和交易所 - 修改 ON CONFLICT 子句以匹配 UNIQUE(userId, exchangeId)
    pub fn bind_user_exchange(&self, user_id: i64, exchange_id: i64, exchange_uid: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO user_exchanges (userId, exchangeId, exchange_uid, isBound)
            VALUES (?1, ?2, ?3, 1)
            ON CONFLICT(userId, exchangeId) DO UPDATE SET exchange_uid = ?3, isBound = 1
            "#,
            params![user_id, exchange_id, exchange_uid],
        )?;
        Ok(())
    }
    // 解绑用户和交易所
    pub fn unbind_user_exchange(&self, user_id: i64, exchange_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE user_exchanges SET isBound = 0 WHERE userId = ? AND exchangeId = ?",
            params![user_id, exchange_id],
        )?;
        Ok(())
    }
    

    // 获取用户数据总览
    pub fn get_user_data(&self, user_id: i64) -> Result<Option<UserData>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT totalMining, totalTradingCost FROM user_data WHERE userId = ?"
        )?;
        stmt.query_row(params![user_id], |row| {
            Ok(UserData {
                total_mining: row.get(0)?,
                total_trading_cost: row.get(1)?,
            })
        }).optional()
    }

    // 更新或插入用户数据总览
    pub fn upsert_user_data(&self, user_id: i64, mining_output: f64, trading_cost: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO user_data (userId, totalMining, totalTradingCost)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(userId) DO UPDATE SET
                totalMining = totalMining + ?2,
                totalTradingCost = totalTradingCost + ?3
            "#,
            params![user_id, mining_output, trading_cost],
        )?;
        Ok(())
    }

    // 获取每日用户数据
    pub fn get_daily_user_data(&self, user_id: i64, date: &str) -> Result<Option<DailyUserData>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT miningOutput, totalTradingCost FROM daily_user_data WHERE userId = ? AND date = ?"
        )?;
        stmt.query_row(params![user_id, date], |row| {
            Ok(DailyUserData {
                mining_output: row.get(0)?,
                total_trading_cost: row.get(1)?,
            })
        }).optional()
    }

    // 新增: 获取用户指定日期范围的每日数据
    pub fn get_daily_user_data_for_range(&self, user_id: i64, start_date: &str, end_date: &str) -> Result<Vec<DailyUserData>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT miningOutput, totalTradingCost FROM daily_user_data WHERE userId = ? AND date BETWEEN ? AND ? ORDER BY date ASC"
        )?;
        let data = stmt.query_map(params![user_id, start_date, end_date], |row| {
            Ok(DailyUserData {
                mining_output: row.get(0)?,
                total_trading_cost: row.get(1)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(data)
    }

    // 更新或插入每日用户数据
    pub fn upsert_daily_user_data(&self, user_id: i64, date: &str, mining_output: f64, total_trading_cost: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO daily_user_data (userId, date, miningOutput, totalTradingCost)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(userId, date) DO UPDATE SET
                miningOutput = miningOutput + ?3,
                totalTradingCost = totalTradingCost + ?4
            "#,
            params![user_id, date, mining_output, total_trading_cost],
        )?;
        Ok(())
    }
    
    // REMOVED: get_level_rebate_percentage is no longer needed.
    // pub fn get_level_rebate_percentage(&self, exp: i64) -> f64 { ... }

    // 获取特定日期的交易记录，以及必要的用户信息
    pub fn get_trades_and_user_info_for_date(&self, trade_date_str: &str) -> Result<Vec<TradeDataForSettlement>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT
                dut.user_id,
                u.email,
                u_inviter.id AS inviter_id,
                dut.exchange_id,
                dut.fee_usdt,
                dut.trade_volume_usdt,
                dut.trade_date
            FROM daily_user_trades dut
            JOIN users u ON dut.user_id = u.id
            LEFT JOIN users u_inviter ON u.inviteBy = u_inviter.email
            WHERE dut.trade_date = ?
            "#
        )?;
        let trades = stmt.query_map(params![trade_date_str], |row| {
            Ok(TradeDataForSettlement {
                user_id: row.get(0)?,
                user_email: row.get(1)?,
                inviter_id: row.get(2).ok(),
                exchange_id: row.get(3)?,
                fee_usdt: row.get(4)?,
                trade_volume_usdt: row.get(5)?,
                trade_date: row.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(trades)
    }

    // 新增: 获取指定日期的所有用户交易记录
    pub fn get_all_daily_user_trades_for_date(&self, date: &str) -> Result<Vec<DailyUserTradeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT
                id, user_id, user_email, exchange_id, exchange_name, trade_volume_usdt, fee_usdt, trade_date, created_at
            FROM daily_user_trades
            WHERE trade_date = ?
            ORDER BY created_at DESC
            "#
        )?;
        let records = stmt.query_map(params![date], |row| {
            Ok(DailyUserTradeRecord {
                id: row.get(0)?,
                user_id: row.get(1)?,
                user_email: row.get(2)?,
                exchange_id: row.get(3)?,
                exchange_name: row.get(4)?,
                trade_volume_usdt: row.get(5)?,
                fee_usdt: row.get(6)?,
                trade_date: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }

    // 在事务中执行整个每日结算 (MODIFIED)
    pub fn perform_daily_settlement(
        &self,
        trade_date_str: &str,
        // The key is user_id, value contains all their earnings for the day (direct + as inviter)
        final_user_earnings: &HashMap<i64, DailyUserRebate>,
        // Commission records to be inserted. Tuple: (inviter_id, invitee_id, amount, currency, date)
        commission_records_to_insert: &Vec<(i64, i64, f64, String, String)>,
        // Platform-wide totals
        total_ntx_distributed_today: f64,
        total_usdt_commission_today: f64, // Sum of all usdt_rebate + usdt_bonus_earned
        active_miners_today: i64,
        total_trading_volume_today: f64
    ) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // 1. 更新用户余额和数据
        for (user_id, earnings) in final_user_earnings {
            let total_ntx_gain = earnings.ntx_rebate + earnings.ntx_bonus_earned;
            let total_usdt_gain = earnings.usdt_rebate + earnings.usdt_bonus_earned;
            let exp_gained = earnings.total_fees_incurred.floor() as i64;

            if total_ntx_gain > 0.0 || total_usdt_gain > 0.0 || exp_gained > 0 {
                tx.execute(
                    "UPDATE users SET ntx_balance = ntx_balance + ?, usdt_balance = usdt_balance + ?, exp = exp + ? WHERE id = ?",
                    params![total_ntx_gain, total_usdt_gain, exp_gained, user_id],
                )?;
            }

            // 只有当用户实际交易时才更新其个人数据
            if earnings.total_fees_incurred > 0.0 {
                // 更新 user_data (总览统计)
                tx.execute(
                    r#"
                    INSERT INTO user_data (userId, totalMining, totalTradingCost)
                    VALUES (?1, ?2, ?3)
                    ON CONFLICT(userId) DO UPDATE SET
                        totalMining = totalMining + ?2,
                        totalTradingCost = totalTradingCost + ?3
                    "#,
                    params![user_id, earnings.ntx_rebate, earnings.total_fees_incurred],
                )?;

                // 更新 daily_user_data (每日数据)
                tx.execute(
                    r#"
                    INSERT INTO daily_user_data (userId, date, miningOutput, totalTradingCost)
                    VALUES (?1, ?2, ?3, ?4)
                    ON CONFLICT(userId, date) DO UPDATE SET
                        miningOutput = miningOutput + ?3,
                        totalTradingCost = totalTradingCost + ?4
                    "#,
                    params![user_id, trade_date_str, earnings.ntx_rebate, earnings.total_fees_incurred],
                )?;
            }
        }

        // 2. 插入佣金记录
        for record in commission_records_to_insert {
            tx.execute(
                "INSERT INTO commission_records (user_id, invited_user_id, commission_amount, commission_currency, record_date) VALUES (?, ?, ?, ?, ?)",
                params![record.0, record.1, record.2, record.3, record.4],
            )?;
        }

        // 3. 更新平台数据
        tx.execute(
            r#"
            INSERT INTO daily_platform_data (date, miningOutput, commission, burned, tradingVolume, miners)
            VALUES (?, ?, ?, 0, ?, ?)
            ON CONFLICT(date) DO UPDATE SET
                miningOutput = excluded.miningOutput,
                commission = excluded.commission,
                burned = excluded.burned,
                tradingVolume = excluded.tradingVolume,
                miners = excluded.miners
            "#,
            params![
                trade_date_str,
                total_ntx_distributed_today,
                total_usdt_commission_today,
                total_trading_volume_today,
                active_miners_today
            ],
        )?;

        tx.execute(
            r#"
            UPDATE platform_data SET
                totalMined = totalMined + ?,
                totalCommission = totalCommission + ?,
                totalTradingVolume = totalTradingVolume + ?,
                platformUsers = (SELECT COUNT(*) FROM users)
            WHERE id = 1
            "#,
            params![
                total_ntx_distributed_today,
                total_usdt_commission_today,
                total_trading_volume_today
            ],
        )?;

        tx.commit()
    }


    // 如果尚未存在，你可能需要一个函数通过邮箱获取用户 ID 以用于推荐奖励
    pub fn get_user_id_by_email(&self, email: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT id FROM users WHERE email = ?", params![email], |row| row.get(0))
            .optional()
    }

    // 获取挖矿排行榜前10名
    pub fn get_mining_leaderboard_top10(&self) -> Result<Vec<MiningLeaderboardEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT
                u.nickname,
                COALESCE(ud.totalMining, 0.0) AS total_mining_amount
            FROM users u
            LEFT JOIN user_data ud ON u.id = ud.userId
            ORDER BY total_mining_amount DESC
            LIMIT 10
            "#
        )?;

        let entries_iter = stmt.query_map([], |row| {
            Ok(MiningLeaderboardEntry {
                rank: 0, // 初始设置为0，将在外部逻辑中填充实际排名
                nickname: row.get(0)?,
                mining_amount: row.get(1)?,
            })
        })?;

        let mut leaderboard: Vec<MiningLeaderboardEntry> = entries_iter.collect::<Result<Vec<_>, _>>()?;

        // 填充排名
        for (i, entry) in leaderboard.iter_mut().enumerate() {
            entry.rank = (i + 1) as i64;
        }

        // 如果不足10人，填充剩余位置为0
        while leaderboard.len() < 10 {
            leaderboard.push(MiningLeaderboardEntry {
                rank: (leaderboard.len() + 1) as i64,
                nickname: "N/A".to_string(),
                mining_amount: 0.0,
            });
        }

        Ok(leaderboard)
    }

    // 获取用户邀请的下级用户
    pub fn get_my_invited_users(&self, user_invite_code: &str) -> Result<Vec<InvitedUserInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, email, nickname FROM users WHERE inviteBy = (SELECT email FROM users WHERE inviteCode = ?)
            "#
        )?;

        let invited_users = stmt.query_map(params![user_invite_code], |row| {
            Ok(InvitedUserInfo {
                id: row.get(0)?,
                email: row.get(1)?,
                nickname: row.get(2)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(invited_users)
    }

    // 获取佣金发放记录
    pub fn get_commission_records(&self, user_id: i64) -> Result<Vec<CommissionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT
                cr.commission_amount,
                cr.commission_currency,
                cr.record_date,
                u.nickname AS invited_user_nickname
            FROM commission_records cr
            JOIN users u ON cr.invited_user_id = u.id
            WHERE cr.user_id = ?
            ORDER BY cr.record_date DESC, cr.created_at DESC
            "#
        )?;

        let records = stmt.query_map(params![user_id], |row| {
            Ok(CommissionRecord {
                amount: row.get(0)?,
                currency: row.get(1)?,
                date: row.get(2)?,
                invited_user_nickname: row.get(3)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    // 新增: 获取所有推荐关系
    pub fn get_all_referral_relationships(&self) -> Result<Vec<ReferralRelationship>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT
                u.id AS inviter_id,
                u.email AS inviter_email,
                ui.id AS invited_user_id,
                ui.nickname AS invited_user_nickname,
                ui.email AS invited_user_email,
                ui.created_at AS invited_at
            FROM users u
            JOIN users ui ON u.email = ui.inviteBy
            ORDER BY u.id, ui.created_at ASC
            "#
        )?;
        let relationships = stmt.query_map([], |row| {
            Ok(ReferralRelationship {
                inviter_id: row.get(0)?,
                inviter_email: row.get(1)?,
                invited_user_id: row.get(2)?,
                invited_user_nickname: row.get(3)?,
                invited_user_email: row.get(4)?,
                invited_at: row.get(5)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(relationships)
    }

    // 新增: 获取所有佣金记录 (管理员用)
    pub fn get_all_commission_records_admin(&self) -> Result<Vec<CommissionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT
                cr.commission_amount,
                cr.commission_currency,
                cr.record_date,
                u.nickname AS invited_user_nickname -- 这里的 nickname 是被邀请人（产生佣金的人）的昵称
            FROM commission_records cr
            JOIN users u ON cr.invited_user_id = u.id
            ORDER BY cr.record_date DESC, cr.created_at DESC
            "#
        )?;
        let records = stmt.query_map([], |row| {
            Ok(CommissionRecord {
                amount: row.get(0)?,
                currency: row.get(1)?,
                date: row.get(2)?,
                invited_user_nickname: row.get(3)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }

    // 新增: 按邀请人汇总佣金数据
    pub fn get_commission_summary_by_inviter(&self) -> Result<Vec<InviterCommissionSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT
                inviter_u.email AS inviter_email, -- 修复：使用 inviter_u.email
                SUM(CASE WHEN cr.commission_currency = 'USDT' THEN cr.commission_amount ELSE 0 END) AS total_usdt_commission,
                SUM(CASE WHEN cr.commission_currency = 'NTX' THEN cr.commission_amount ELSE 0 END) AS total_ntx_commission
            FROM commission_records cr
            JOIN users inviter_u ON cr.user_id = inviter_u.id -- cr.user_id 是邀请人
            LEFT JOIN users invited_u ON cr.invited_user_id = invited_u.id
            GROUP BY inviter_email
            ORDER BY total_usdt_commission DESC
            "#
        )?;
        let summary = stmt.query_map([], |row| {
            Ok(InviterCommissionSummary {
                inviter_email: row.get(0)?,
                total_usdt_commission: row.get(1)?,
                total_ntx_commission: row.get(2)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(summary)
    }


    //管理员部分

    // 获取所有用户信息
    pub fn get_all_users(&self) -> Result<Vec<UserInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, nickname, email, inviteCode, inviteBy, exp, usdt_balance, ntx_balance, is_active, gntx_balance FROM users"
        )?;
        let user_iter = stmt.query_map([], |row| {
            Ok(UserInfo {
                id: row.get(0)?,
                nickname: row.get(1)?,
                email: row.get(2)?,
                my_invite_code: row.get(3)?,
                invited_by: row.get(4)?,
                exp: row.get(5)?,
                usdt_balance: row.get(6)?,
                ntx_balance: row.get(7)?,
                is_active: row.get(8)?,
                gntx_balance: row.get(9)?,
            })
        })?;

        let mut users = Vec::new();
        for user in user_iter {
            users.push(user?);
        }
        Ok(users)
    }

    // 获取用户邮箱
    pub fn get_user_email_by_id(&self, user_id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT email FROM users WHERE id = ?", params![user_id], |row| row.get(0))
            .optional()
    }

    // 获取交易所名称
    pub fn get_exchange_name_by_id(&self, exchange_id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT name FROM exchanges WHERE id = ?", params![exchange_id], |row| row.get(0))
            .optional()
    }

    // 添加或更新用户每日交易数据
    pub fn add_or_update_daily_trade_data(&self, user_id: i64, user_email: String, exchange_id: i64, exchange_name: String, trade_volume_usdt: f64, fee_usdt: f64, trade_date: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            r#"
            INSERT INTO daily_user_trades (user_id, user_email, exchange_id, exchange_name, trade_volume_usdt, fee_usdt, trade_date)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(user_id, exchange_id, trade_date) DO UPDATE SET
                trade_volume_usdt = daily_user_trades.trade_volume_usdt + excluded.trade_volume_usdt,
                fee_usdt = daily_user_trades.fee_usdt + excluded.fee_usdt
            "#,
            params![user_id, user_email, exchange_id, exchange_name, trade_volume_usdt, fee_usdt, trade_date],
        )?;
        Ok(())
    }

    // 更新交易所挖矿效率
    pub fn update_exchange_mining_efficiency(&self, exchange_id: i64, new_efficiency: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE exchanges SET miningEfficiency = ? WHERE id = ?",
            params![new_efficiency, exchange_id],
        )?;
        Ok(())
    }

    // 更新用户激活状态 (封禁/解封)
    pub fn update_user_active_status(&self, user_id: i64, is_active: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET is_active = ? WHERE id = ?",
            params![is_active, user_id],
        )?;
        Ok(())
    }

    // 获取所有提现订单
    pub fn get_all_withdrawal_orders(&self) -> Result<Vec<WithdrawalOrder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, user_email, amount, currency, to_address, is_confirmed, created_at, processed_at, status FROM withdrawal_orders ORDER BY created_at DESC",
        )?;
        let withdrawal_order_iter = stmt.query_map([], |row| {
            Ok(WithdrawalOrder {
                id: row.get(0)?,
                user_id: row.get(1)?,
                user_email: row.get(2)?,
                amount: row.get(3)?,
                currency: row.get(4)?,
                to_address: row.get(5)?,
                is_confirmed: row.get(6)?,
                created_at: row.get(7)?,
                processed_at: row.get(8)?,
                status: row.get(9)?,
            })
        })?;

        let mut orders = Vec::new();
        for order in withdrawal_order_iter {
            orders.push(order?);
        }
        Ok(orders)
    }

    // 获取用户自己的提现订单
    pub fn get_user_withdrawal_orders(&self, user_id: i64) -> Result<Vec<WithdrawalOrder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, user_id, user_email, amount, currency, to_address, is_confirmed, created_at, processed_at, status FROM withdrawal_orders WHERE user_id = ? ORDER BY created_at DESC",
        )?;
        let withdrawal_order_iter = stmt.query_map(params![user_id], |row| {
            Ok(WithdrawalOrder {
                id: row.get(0)?,
                user_id: row.get(1)?,
                user_email: row.get(2)?,
                amount: row.get(3)?,
                currency: row.get(4)?,
                to_address: row.get(5)?,
                is_confirmed: row.get(6)?,
                created_at: row.get(7)?,
                processed_at: row.get(8)?,
                status: row.get(9)?,
            })
        })?;

        let mut orders = Vec::new();
        for order in withdrawal_order_iter {
            orders.push(order?);
        }
        Ok(orders)
    }

    // 更新提现订单状态
    pub fn update_withdrawal_order_status(&self, order_id: i64, status: &str, processed_at: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE withdrawal_orders SET status = ?, processed_at = ?, is_confirmed = ? WHERE id = ?",
            params![status, processed_at, status == "approved", order_id],
        )?;
        Ok(())
    }

    // 新增: 获取财务汇总数据
    pub fn get_financial_summary(&self) -> Result<FinancialSummary> {
        let conn = self.conn.lock().unwrap();
        
        // 总 USDT 和 NTX 在用户余额中
        let (total_usdt_in_system, total_ntx_in_system): (f64, f64) = conn.query_row(
            "SELECT SUM(usdt_balance), SUM(ntx_balance) FROM users",
            [],
            |row| Ok((row.get(0).unwrap_or(0.0), row.get(1).unwrap_or(0.0))),
        )?;

        // 提现订单计数和金额汇总
        let (pending_withdrawals_count, approved_withdrawals_count, rejected_withdrawals_count): (i64, i64, i64) = conn.query_row(
            "SELECT
                SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status = 'approved' THEN 1 ELSE 0 END),
                SUM(CASE WHEN status = 'rejected' THEN 1 ELSE 0 END)
            FROM withdrawal_orders",
            [],
            |row| Ok((row.get(0).unwrap_or(0), row.get(1).unwrap_or(0), row.get(2).unwrap_or(0))),
        )?;

        let (total_usdt_withdrawn, total_ntx_withdrawn): (f64, f64) = conn.query_row(
            "SELECT
                SUM(CASE WHEN currency = 'USDT' AND status = 'approved' THEN amount ELSE 0 END),
                SUM(CASE WHEN currency = 'NTX' AND status = 'approved' THEN amount ELSE 0 END)
            FROM withdrawal_orders",
            [],
            |row| Ok((row.get(0).unwrap_or(0.0), row.get(1).unwrap_or(0.0))),
        )?;

        Ok(FinancialSummary {
            total_usdt_in_system,
            total_ntx_in_system,
            pending_withdrawals_count,
            approved_withdrawals_count,
            rejected_withdrawals_count,
            total_usdt_withdrawn,
            total_ntx_withdrawn,
        })
    }

    // 更新用户总数据 (totalMining, totalTradingCost)
    pub fn update_user_total_data(&self, user_id: i64, total_mining: f64, total_trading_cost: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE user_data SET totalMining = ?, totalTradingCost = ? WHERE userId = ?",
            params![total_mining, total_trading_cost, user_id],
        )?;
        Ok(())
    }

    // 更新每日用户数据 (miningOutput, totalTradingCost)
    pub fn update_daily_user_data_by_admin(&self, user_id: i64, date: &str, mining_output: f64, total_trading_cost: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE daily_user_data SET miningOutput = ?, totalTradingCost = ? WHERE userId = ? AND date = ?",
            params![mining_output, total_trading_cost, user_id, date],
        )?;
        Ok(())
    }

    // 更新平台总数据
    pub fn update_platform_total_data(&self, total_mined: f64, total_commission: f64, total_burned: f64, total_trading_volume: f64, platform_users: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE platform_data SET totalMined = ?, totalCommission = ?, totalBurned = ?, totalTradingVolume = ?, platformUsers = ? WHERE id = 1",
            params![total_mined, total_commission, total_burned, total_trading_volume, platform_users],
        )?;
        Ok(())
    }

    // 更新每日平台数据
    pub fn update_daily_platform_data_by_admin(&self, date: &str, mining_output: f64, burned: f64, commission: f64, trading_volume: f64, miners: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE daily_platform_data SET miningOutput = ?, burned = ?, commission = ?, tradingVolume = ?, miners = ? WHERE date = ?",
            params![mining_output, burned, commission, trading_volume, miners, date],
        )?;
        Ok(())
    }

    // 修改用户个人信息
    pub fn update_user_profile(&self, user_id: i64, nickname: &str, email: &str, invite_code: &str, exp: i64, usdt_balance: f64, ntx_balance: f64, is_active: bool,is_admin: bool,is_broker: bool) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET nickname = ?, email = ?, inviteCode = ?, exp = ?, usdt_balance = ?, ntx_balance = ?, is_active = ?,is_admin = ?,is_broker = ? WHERE id = ?",
            params![nickname, email, invite_code, exp, usdt_balance, ntx_balance, is_active, is_admin, is_broker, user_id],
        )?;
        Ok(())
    }

    // DAO 拍卖相关操作 (新增)

    // 创建 DAO 拍卖 
    pub fn create_dao_auction(&self, admin_bsc_address: &str, start_time: &str, end_time: &str) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        // 检查是否有正在进行的拍卖
        let active_auction_count: i64 = tx.query_row(
            "SELECT COUNT(*) FROM dao_auctions WHERE is_active = 1",
            [],
            |row| row.get(0),
        )?;

        if active_auction_count > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                ffi::Error::new(ffi::SQLITE_MISUSE),
                Some("当前已有正在进行的DAO拍卖，不能同时存在多个拍卖".to_string()),
            ));
        }

        tx.execute(
            "INSERT INTO dao_auctions (admin_bsc_address, start_time, end_time, is_active) VALUES (?, ?, ?, 1)",
            params![admin_bsc_address, start_time, end_time],
        )?;
        tx.commit()
    }

    // 结束 DAO 拍卖
    pub fn end_dao_auction(&self) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE dao_auctions SET is_active = 0 WHERE is_active = 1",
            [],
        )?;
        tx.commit()
    }

    // 获取当前正在进行的 DAO 拍卖
    pub fn get_current_dao_auction(&self) -> Result<Option<DaoAuction>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, admin_bsc_address, start_time, end_time, is_active FROM dao_auctions WHERE is_active = 1 ORDER BY start_time DESC LIMIT 1"
        )?;
        let current_auction = stmt.query_row([], |row| {
            Ok(DaoAuction {
                id: row.get(0)?,
                admin_bsc_address: row.get(1)?,
                start_time: row.get(2)?,
                end_time: row.get(3)?,
                is_active: row.get(4)?,
            })
        }).optional()?;

        // 如果存在拍卖，检查其是否已过期
        if let Some(auction) = current_auction {
            let current_utc = Utc::now().to_rfc3339();
            if current_utc >= auction.end_time {
                let _ = self.end_dao_auction();
                return Ok(None);
            }
            Ok(Some(auction))
        } else {
            Ok(None)
        }
    }

    // 新增: 获取所有 DAO 拍卖历史 (管理员用)
    pub fn get_all_dao_auctions(&self) -> Result<Vec<DaoAuction>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, admin_bsc_address, start_time, end_time, is_active FROM dao_auctions ORDER BY start_time DESC"
        )?;
        let auctions = stmt.query_map([], |row| {
            Ok(DaoAuction {
                id: row.get(0)?,
                admin_bsc_address: row.get(1)?,
                start_time: row.get(2)?,
                end_time: row.get(3)?,
                is_active: row.get(4)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(auctions)
    }

    // 绑定用户 BSC 地址
    pub fn bind_user_bsc_address(&self, user_id: i64, bsc_address: &str, bound_at: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO user_bsc_addresses (user_id, bsc_address, bound_at) VALUES (?, ?, ?)",
            params![user_id, bsc_address, bound_at],
        )?;
        Ok(())
    }

    // 获取所有用户绑定的 BSC 地址
    pub fn get_all_user_bsc_addresses(&self) -> Result<Vec<UserBscAddressInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT
                uba.user_id,
                u.nickname,
                u.email,
                uba.bsc_address,
                uba.bound_at
            FROM user_bsc_addresses uba
            JOIN users u ON uba.user_id = u.id
            "#
        )?;
        let addresses = stmt.query_map([], |row| {
            Ok(UserBscAddressInfo {
                user_id: row.get(0)?,
                nickname: row.get(1)?,
                email: row.get(2)?,
                bsc_address: row.get(3)?,
                bound_at: row.get(4)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(addresses)
    }

    // 获取特定用户的 BSC 地址
    pub fn get_user_bsc_address(&self, user_id: i64) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT bsc_address FROM user_bsc_addresses WHERE user_id = ?", params![user_id], |row| row.get(0))
            .optional()
    }

    // 创建学院文章
    pub fn create_academy_article(&self, title: &str, summary: &str, image_url: Option<&str>, is_displayed: bool, content: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let publish_date = Utc::now().to_rfc3339();
        let modify_date = publish_date.clone();
        conn.execute(
            "INSERT INTO academy_articles (title, summary, image_url, publish_date, modify_date, is_displayed, content) VALUES (?, ?, ?, ?, ?, ?, ?)",
            params![title, summary, image_url, publish_date, modify_date, is_displayed, content],
        )?;
        Ok(conn.last_insert_rowid())
    }

    // 更新学院文章
    pub fn update_academy_article(&self, id: i64, title: &str, summary: &str, image_url: Option<&str>, is_displayed: bool, content: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let modify_date = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE academy_articles SET title = ?, summary = ?, image_url = ?, modify_date = ?, is_displayed = ?, content = ? WHERE id = ?",
            params![title, summary, image_url, modify_date, is_displayed, content, id],
        )?;
        Ok(())
    }

    // 删除学院文章
    pub fn delete_academy_article(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM academy_articles WHERE id = ?", params![id])?;
        Ok(())
    }

    // 获取所有学院文章（用户端使用，只获取 is_displayed 为 true 的文章）
    pub fn get_all_academy_articles(&self, only_displayed: bool) -> Result<Vec<AcademyArticleSummary>> {
        let conn = self.conn.lock().unwrap();
        let mut query = "SELECT id, title, summary, image_url, publish_date, modify_date, is_displayed FROM academy_articles".to_string();
        
        if only_displayed {
            query.push_str(" WHERE is_displayed = 1");
        }
        query.push_str(" ORDER BY publish_date DESC");

        let mut stmt = conn.prepare(&query)?;

        let articles = stmt.query_map([], |row| {
            Ok(AcademyArticleSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                image_url: row.get(3)?,
                publish_date: row.get(4)?,
                modify_date: row.get(5)?,
                is_displayed: row.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(articles)
    }

    // 管理员获取所有学院文章（包括未展示的文章）
    pub fn get_all_academy_articles_admin(&self) -> Result<Vec<AcademyArticleSummary>> {
        let conn = self.conn.lock().unwrap();
        let query = "SELECT id, title, summary, image_url, publish_date, modify_date, is_displayed FROM academy_articles ORDER BY publish_date DESC";
        
        let mut stmt = conn.prepare(query)?;

        let articles = stmt.query_map([], |row| {
            Ok(AcademyArticleSummary {
                id: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                image_url: row.get(3)?,
                publish_date: row.get(4)?,
                modify_date: row.get(5)?,
                is_displayed: row.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(articles)
    }

    // 根据 ID 获取学院文章详情
    pub fn get_academy_article_by_id(&self, id: i64) -> Result<Option<AcademyArticle>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, summary, image_url, publish_date, modify_date, is_displayed, content FROM academy_articles WHERE id = ?"
        )?;
        stmt.query_row(params![id], |row| {
            Ok(AcademyArticle {
                id: row.get(0)?,
                title: row.get(1)?,
                summary: row.get(2)?,
                image_url: row.get(3)?,
                publish_date: row.get(4)?,
                modify_date: row.get(5)?,
                is_displayed: row.get(6)?,
                content: row.get(7)?,
            })
        }).optional()
    }

    // 更新用户昵称
    pub fn update_user_nickname(&self, user_id: i64, new_nickname: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET nickname = ? WHERE id = ?",
            params![new_nickname, user_id],
        )?;
        Ok(())
    }

    // 根据用户ID更新用户密码
    pub fn update_user_password_by_id(&self, user_id: i64, new_hashed_password: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET password = ? WHERE id = ?",
            params![new_hashed_password, user_id],
        )?;
        Ok(())
    }
    // 新增: 获取所有用户及其绑定的 BSC 地址和 GNTX 数量
    pub fn get_all_user_bsc_addresses_with_gntx(&self) -> Result<Vec<UserGNTXInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT 
                u.email, 
                uba.bsc_address, 
                u.gntx_balance 
            FROM users u
            LEFT JOIN user_bsc_addresses uba ON u.id = uba.user_id;
            "#
        )?;
        
        let user_info_iter = stmt.query_map([], |row| {
            Ok(UserGNTXInfo {
                email: row.get(0)?,
                bsc_address: row.get(1)?,
                gntx_balance: row.get(2)?,
            })
        })?;

        let mut user_info_list = Vec::new();
        for user_info in user_info_iter {
            user_info_list.push(user_info?);
        }
        Ok(user_info_list)
    }

    // 根据用户邮箱更新 GNTX 数量
    pub fn update_user_gntx_balance_by_email(&self, email: &str, gntx_balance: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE users SET gntx_balance = ? WHERE email = ?",
            params![gntx_balance, email],
        )?;
        Ok(())
    }

    // 获取指定交易所下所有用户绑定的 UID 列表
    pub fn get_exchange_bound_users(&self, exchange_id: i64) -> Result<Vec<UserExchangeBindingInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT
                ue.exchange_uid,
                ue.userId -- 修改此处，选择 user_exchanges 表中的 userId
            FROM user_exchanges ue
            WHERE ue.exchangeId = ? AND ue.isBound = 1
            "#
        )?;
        let users = stmt.query_map(params![exchange_id], |row| {
            Ok(UserExchangeBindingInfo {
                exchange_uid: row.get(0)?,
                user_id: row.get(1)?, // 映射到修改后的 user_id 字段
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(users)
    }
    // 账号是否激活
    pub fn is_user_active(&self, user_id: i64) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT is_active FROM users WHERE id = ?",
            params![user_id],
            |row| row.get(0)
        )
    }
// 新增：获取NTX分配控制的设置


    // 新增：获取NTX分配控制的目标百分比
    // pub fn get_ntx_control_percentage(&self) -> Result<f64> {
    //     let conn = self.conn.lock().unwrap();
    //     // 如果表或值不存在，默认为90.0
    //     conn.query_row(
    //         "SELECT admin_fee_percentage FROM ntx_control_settings WHERE id = 1",
    //         [],
    //         |row| row.get(0),
    //     )
    //     .optional()
    //     .map(|res| res.unwrap_or(Ok(90.0)))? // Provide a default if the query returns no rows
    //     .map_err(|e| e.into())
    // }
    // pub fn get_ntx_control_percentage(&self) -> Result<f64> {
    //     let conn = self.conn.lock().unwrap();
    //     // 如果表或值不存在，默认为90.0
    //     conn.query_row(
    //         "SELECT admin_fee_percentage FROM ntx_control_settings WHERE id = 1",
    //         [],
    //         |row| Ok(row.get(0)?), // <-- 修改1：使用 '?' 解包 row.get(0)，然后用 Ok() 包装解包后的 f64
    //     )
    //     .optional()
    //     .map(|res| res.unwrap_or(90.0))? // <-- 修改2：现在 res 是 Option<f64>，直接 unwrap_or(90.0) 得到 f64
    //     .map_err(|e| e.into())
    // }//fixed
    pub fn get_ntx_control_percentage(&self) -> Result<f64> {
    let conn = self.conn.lock().unwrap();
    // 如果表或值不存在，默认为90.0
    conn.query_row(
        "SELECT admin_fee_percentage FROM ntx_control_settings WHERE id = 1",
        [],
        |row| Ok(row.get(0)?), // 闭包返回 Result<f64, rusqlite::Error>
    )
    .optional() // 返回 Result<Option<f64>, rusqlite::Error>
    .map(|res| res.unwrap_or(90.0)) // 返回 Result<f64, rusqlite::Error>
    // <-- 在这里不再需要 .map_err()，因为最终的 Result 会被 ? 操作符处理
} // 函数的返回值是 Result<f64>，这里隐式返回了上面链式调用的 Result<f64, rusqlite::Error>

    // 新增：更新NTX分配控制的目标百分比 (用于Admin后台)
    pub fn update_ntx_control_percentage(&self, percentage: f64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE ntx_control_settings SET admin_fee_percentage = ? WHERE id = 1",
            params![percentage],
        )?;
        Ok(())
    }

    // 新增：获取所有管理员用户的ID
    pub fn get_all_admin_user_ids(&self) -> Result<Vec<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM users WHERE is_admin = TRUE")?;
        let ids = stmt.query_map([], |row| row.get(0))?
                    .collect::<Result<Vec<i64>, _>>()?;
        Ok(ids)
    }

    // 新增：获取指定日期的总手续费（可按是否为管理员筛选）
    pub fn get_total_fees_for_date(&self, trade_date: &str, admins_only: bool) -> Result<f64> {
        let conn = self.conn.lock().unwrap();
        let sql = if admins_only {
            r#"
            SELECT COALESCE(SUM(dut.fee_usdt), 0.0)
            FROM daily_user_trades dut
            JOIN users u ON dut.user_id = u.id
            WHERE dut.trade_date = ? AND u.is_admin = TRUE
            "#
        } else {
            "SELECT COALESCE(SUM(fee_usdt), 0.0) FROM daily_user_trades WHERE trade_date = ?"
        };
        let total_fees: f64 = conn.query_row(sql, params![trade_date], |row| row.get(0))?;
        Ok(total_fees)
    }

    // 新增: 在一个事务中批量添加虚假的管理员交易数据
    pub fn add_fake_admin_trades_in_transaction(&self, trades: &[FakeTradeData]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        for trade in trades {
            // 使用 ON CONFLICT 来累加费用，这与 add_or_update_daily_trade_data 逻辑一致
            tx.execute(
                r#"
                INSERT INTO daily_user_trades (user_id, user_email, exchange_id, exchange_name, trade_volume_usdt, fee_usdt, trade_date)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(user_id, exchange_id, trade_date) DO UPDATE SET
                    trade_volume_usdt = daily_user_trades.trade_volume_usdt + excluded.trade_volume_usdt,
                    fee_usdt = daily_user_trades.fee_usdt + excluded.fee_usdt
                "#,
                params![&trade.user_id, &trade.user_email, &trade.exchange_id, &trade.exchange_name, &trade.trade_volume_usdt, &trade.fee_usdt, &trade.trade_date],
            )?;
        }

        tx.commit()
    }
    // 检查用户是否有在指定日期有交易记录的直属下级
    // pub fn has_invited_user_with_trade_on_date(&self, inviter_email: &str, trade_date: &str) -> Result<bool> {
    //     let conn = self.conn.lock().unwrap();
    //     // 查询是否有直属下级在指定日期有交易记录
    //     let count: i64 = conn.query_row(
    //         r#"
    //         SELECT COUNT(DISTINCT dut.user_id)
    //         FROM users u_invited
    //         JOIN daily_user_trades dut ON u_invited.id = dut.user_id
    //         WHERE u_invited.inviteBy = ? AND dut.trade_date = ?
    //         LIMIT 1
    //         "#,
    //         params![inviter_email, trade_date],
    //         |row| row.get(0),
    //     )?;
    //     Ok(count > 0)
    // }
}
// 交易所信息结构体
#[derive(Debug, Serialize)]
pub struct ExchangeInfo {
    pub id: i64,
    pub name: String,
    pub logo_url: String,
    pub mining_efficiency: f64,
    pub cex_url: String,
}

#[derive(Debug)]
pub struct PlatformData {
    pub total_mined: f64,
    pub total_commission: f64,
    pub total_burned: f64,
    pub total_trading_volume: f64,
    pub platform_users: i64,
    pub genesis_date: String,
}

#[derive(Debug)]
pub struct DailyPlatformData {
    pub mining_output: f64,
    pub burned: f64,
    pub commission: f64,
    pub trading_volume: f64,
    pub miners: i64,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: i64,
    pub nickname: String,
    pub email: String,
    pub my_invite_code: String,
    pub invited_by: Option<String>,
    pub exp: i64,
    pub usdt_balance: f64,
    pub ntx_balance: f64,
    pub is_active: bool,
    pub gntx_balance: f64,
}

//  用户完整信息结构体 (用于管理员)
#[derive(Debug, Serialize)]
pub struct UserFullInfo {
    pub id: i64,
    pub email: String,
    pub nickname: String,
    #[serde(rename = "passwordHash")]
    pub password_hash: String, 
    #[serde(rename = "myInviteCode")]
    pub my_invite_code: String,
    #[serde(rename = "invitedBy")]
    pub invited_by: Option<String>,
    pub exp: i64,
    #[serde(rename = "usdtBalance")]
    pub usdt_balance: f64,
    #[serde(rename = "ntxBalance")]
    pub ntx_balance: f64,
    #[serde(rename = "isActive")]
    pub is_active: bool,
    #[serde(rename = "isAdmin")]
    pub is_admin: bool,
    #[serde(rename = "isBroker")]
    pub is_broker: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct UserData {
    pub total_mining: f64,
    pub total_trading_cost: f64, 
}

#[derive(Debug, Serialize)]
pub struct DailyUserData {
    pub mining_output: f64,
    pub total_trading_cost: f64, 
}

// MODIFIED: This struct now holds inviter_id instead of inviter_email and no longer needs current_exp
#[derive(Debug)]
pub struct TradeDataForSettlement {
    pub user_id: i64,
    pub user_email: String,
    pub inviter_id: Option<i64>,
    pub exchange_id: i64,
    pub fee_usdt: f64,
    pub trade_volume_usdt: f64,
    pub trade_date: String,
}

// MODIFIED: This struct is updated to hold all earnings for a user for a given day.
#[derive(Debug, Default, Clone)]
pub struct DailyUserRebate {
    pub ntx_rebate: f64,              // User's own NTX from trading
    pub usdt_rebate: f64,             // User's own USDT from trading
    pub ntx_bonus_earned: f64,        // NTX bonus earned as an inviter
    pub usdt_bonus_earned: f64,       // USDT bonus earned as an inviter
    pub total_fees_incurred: f64,     // Total fees the user generated (for logging and EXP)
}


#[derive(Debug, Serialize)]
pub struct WithdrawalOrder {
    pub id: i64,
    pub user_id: i64,
    pub user_email: String,
    pub amount: f64,
    pub currency: String,
    pub to_address: String,
    pub is_confirmed: bool,
    pub created_at: String,
    pub processed_at: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct MiningLeaderboardEntry {
    pub rank: i64,
    pub nickname: String,
    pub mining_amount: f64,
}

#[derive(Debug, Serialize)] 
pub struct InvitedUserInfo {
    pub id: i64, 
    pub email: String,
    pub nickname: String,
}

#[derive(Debug, Serialize)]
pub struct CommissionRecord {
    pub amount: f64,
    pub currency: String,
    #[serde(rename = "date")]
    pub date: String,
    #[serde(rename = "invitedUserNickname")]
    pub invited_user_nickname: String,
}

#[derive(Debug, Serialize)]
pub struct DaoAuction {
    pub id: i64,
    pub admin_bsc_address: String,
    pub start_time: String,
    pub end_time: String,
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
pub struct UserBscAddressInfo {
    pub user_id: i64,
    pub nickname: String,
    pub email: String,
    pub bsc_address: String,
    pub bound_at: String,
}

#[derive(Debug, Serialize)]
pub struct UserGNTXInfo {
    pub email: String,
    #[serde(rename = "bscAddress")]
    pub bsc_address: Option<String>,
    #[serde(rename = "gntxBalance")]
    pub gntx_balance: f64,
}



// 管理员仪表盘数据结构体
#[derive(Debug, Serialize)]
pub struct AdminDashboardData {
    pub pending_withdrawals: i64,
    pub new_users_today: i64,
    // 新增平台总数据字段
    #[serde(rename = "totalMined")]
    pub total_mined: f64,
    #[serde(rename = "totalCommission")]
    pub total_commission: f64,
    #[serde(rename = "totalBurned")]
    pub total_burned: f64,
    #[serde(rename = "totalTradingVolume")]
    pub total_trading_volume: f64,
    #[serde(rename = "platformUsers")]
    pub platform_users: i64,
}

// 学院文章结构体 (完整版)
#[derive(Debug, Serialize)]
pub struct AcademyArticle {
    pub id: i64,
    pub title: String,
    pub summary: String,
    #[serde(rename = "imageUrl")]
    pub image_url: Option<String>,
    #[serde(rename = "publishDate")]
    pub publish_date: String,
    #[serde(rename = "modifyDate")]
    pub modify_date: String,
    #[serde(rename = "isDisplayed")]
    pub is_displayed: bool,
    pub content: String,
}

// 学院文章摘要结构体 (不包含 content)
#[derive(Debug, Serialize)]
pub struct AcademyArticleSummary {
    pub id: i64,
    pub title: String,
    pub summary: String,
    #[serde(rename = "imageUrl")]
    pub image_url: Option<String>,
    #[serde(rename = "publishDate")]
    pub publish_date: String,
    #[serde(rename = "modifyDate")]
    pub modify_date: String,
    #[serde(rename = "isDisplayed")]
    pub is_displayed: bool,
}

// 新增: 历史平台数据结构体
#[derive(Debug, Serialize)]
pub struct HistoricalPlatformData {
    pub date: String,
    #[serde(rename = "miningOutput")]
    pub mining_output: f64,
    pub burned: f64,
    pub commission: f64,
    #[serde(rename = "tradingVolume")]
    pub trading_volume: f64,
    pub miners: i64,
}

// 新增: 每日用户交易记录结构体
#[derive(Debug, Serialize)]
pub struct DailyUserTradeRecord {
    pub id: i64,
    #[serde(rename = "userId")]
    pub user_id: i64,
    #[serde(rename = "userEmail")]
    pub user_email: String,
    #[serde(rename = "exchangeId")]
    pub exchange_id: i64,
    #[serde(rename = "exchangeName")]
    pub exchange_name: String,
    #[serde(rename = "tradeVolumeUsdt")]
    pub trade_volume_usdt: f64,
    #[serde(rename = "feeUsdt")]
    pub fee_usdt: f64,
    #[serde(rename = "tradeDate")]
    pub trade_date: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

// 新增: 推荐关系结构体
#[derive(Debug, Serialize)]
pub struct ReferralRelationship {
    #[serde(rename = "inviterId")]
    pub inviter_id: i64,
    #[serde(rename = "inviterEmail")]
    pub inviter_email: String,
    #[serde(rename = "invitedUserId")]
    pub invited_user_id: i64,
    #[serde(rename = "invitedUserNickname")]
    pub invited_user_nickname: String,
    #[serde(rename = "invitedUserEmail")]
    pub invited_user_email: String,
    #[serde(rename = "invitedAt")]
    pub invited_at: String,
}

// 新增: 邀请人佣金汇总结构体
#[derive(Debug, Serialize)]
pub struct InviterCommissionSummary {
    #[serde(rename = "inviterEmail")]
    pub inviter_email: String,
    #[serde(rename = "totalUsdtCommission")]
    pub total_usdt_commission: f64,
    #[serde(rename = "totalNtxCommission")]
    pub total_ntx_commission: f64,
}

// 新增: 财务汇总结构体
#[derive(Debug, Serialize)]
pub struct FinancialSummary {
    #[serde(rename = "totalUsdtInSystem")]
    pub total_usdt_in_system: f64,
    #[serde(rename = "totalNtxInSystem")]
    pub total_ntx_in_system: f64,
    #[serde(rename = "pendingWithdrawalsCount")]
    pub pending_withdrawals_count: i64,
    #[serde(rename = "approvedWithdrawalsCount")]
    pub approved_withdrawals_count: i64,
    #[serde(rename = "rejectedWithdrawalsCount")]
    pub rejected_withdrawals_count: i64,
    #[serde(rename = "totalUsdtWithdrawn")]
    pub total_usdt_withdrawn: f64,
    #[serde(rename = "totalNtxWithdrawn")]
    pub total_ntx_withdrawn: f64,
}

#[derive(Debug, Serialize)]
pub struct UserExchangeBindingInfo {
    #[serde(rename = "exchangeUid")]
    pub exchange_uid: String,
    #[serde(rename = "userId")] 
    pub user_id: i64,
}

#[derive(Debug)]
pub struct FakeTradeData {
    pub user_id: i64,
    pub user_email: String,
    pub exchange_id: i64,
    pub exchange_name: String,
    pub trade_volume_usdt: f64,
    pub fee_usdt: f64,
    pub trade_date: String,
}