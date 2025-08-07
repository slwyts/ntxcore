use ethers::prelude::*;
use std::sync::Arc;
use actix_web::web::Data;
use crate::db::Database;
use crate::admin::{db_get_all_user_gntx_info, db_update_user_gntx_balance};
use tokio::time::{sleep, Duration};

// GNTX NFT 合约 ABI（只包含需要的部分）
abigen!(
    GntxNftContract,
    r#"[
        event NFTMinted(address indexed user, uint256 indexed tokenId, uint256 amount)
        event NFTBurned(address indexed user, uint256 indexed tokenId, uint256 amount)
        function userBindings(address) view returns (string)
        function balanceOf(address account, uint256 id) view returns (uint256)
    ]"#
);

pub async fn start_gntx_sync(db: Data<Database>) {
    // 读取环境变量
    let bsc_provider_url = std::env::var("BSC_PROVIDER_URL").unwrap_or_else(|_| "https://data-seed-prebsc-1-s1.binance.org:8545/".to_string());
    let gntx_contract_addr = match std::env::var("GNTX_NFT_CONTRACT_ADDRESS") {
        Ok(addr) => addr,
        Err(_) => {
            eprintln!("GNTX_NFT_CONTRACT_ADDRESS 未设置，跳过 GNTX 链上同步");
            return;
        }
    };
    let provider = match Provider::<Http>::try_from(bsc_provider_url.clone()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("BSC Provider 初始化失败: {}", e);
            return;
        }
    };
    let provider = Arc::new(provider);
    let contract = GntxNftContract::new(gntx_contract_addr.parse::<Address>().unwrap(), provider.clone());

    // 启动时全量同步
    let db_clone = db.clone();
    let contract_clone = contract.clone();
    tokio::spawn(async move {
        loop {
            if let Err(e) = initial_sync(&db_clone, contract_clone.clone()).await {
                eprintln!("GNTX 初始同步失败: {}", e);
            }
            sleep(Duration::from_secs(60 * 60)).await; // 每小时同步一次
        }
    });

    // 监听事件
    let db_clone = db.clone();
    let contract_clone = contract.clone();
    tokio::spawn(async move {
        let minted_filter = contract_clone
            .event::<NftmintedFilter>()
            .from_block(0u64);
        let mut minted_stream = minted_filter.stream().await.unwrap();
        let burned_filter = contract_clone
            .event::<NftburnedFilter>()
            .from_block(0u64);
        let mut burned_stream = burned_filter.stream().await.unwrap();

        loop {
            tokio::select! {
                Some(Ok(event)) = minted_stream.next() => {
                    handle_event(event.user, &contract_clone, &db_clone).await;
                },
                Some(Ok(event)) = burned_stream.next() => {
                    handle_event(event.user, &contract_clone, &db_clone).await;
                },
                else => sleep(Duration::from_secs(5)).await,
            }
        }
    });
}

async fn initial_sync(db: &Database, contract: GntxNftContract<Provider<Http>>) -> Result<(), Box<dyn std::error::Error>> {
    // 获取所有用户信息
    let users = db_get_all_user_gntx_info(db)?;
    for user in users {
        let bsc_address = user.bsc_address;
        let email = user.email;
        if let Some(addr) = bsc_address {
            let onchain_balance = contract.balance_of(addr.parse()?, U256::from(1u64)).call().await?;
            let onchain_balance = onchain_balance.as_u64() as f64;
            if (onchain_balance - user.gntx_balance).abs() > 0.01 {
                let _ = db_update_user_gntx_balance(db, &email, onchain_balance);
            }
        }
    }
    Ok(())
}

async fn handle_event(user_addr: Address, contract: &GntxNftContract<Provider<Http>>, db: &Database) {
    // 查询绑定邮箱
    match contract.user_bindings(user_addr).call().await {
        Ok(email) if !email.is_empty() => {
            // 查询链上余额
            match contract.balance_of(user_addr, U256::from(1u64)).call().await {
                Ok(onchain_balance) => {
                    let _ = db_update_user_gntx_balance(db, &email, onchain_balance.as_u64() as f64);
                },
                Err(e) => eprintln!("查询链上余额失败: {}", e),
            }
        },
        _ => eprintln!("未找到绑定邮箱，跳过同步"),
    }
}
