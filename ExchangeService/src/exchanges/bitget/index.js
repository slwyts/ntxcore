const moment = require('moment');
const EasyAPI = require('./easyapi');
const backendApiService = require('../../services/backendApiService');
const stateService = require('../../services/stateService');
const logger = require('../../utils/logger');

const EXCHANGE_NAME = 'Bitget';
const EXCHANGE_IDENTIFIER = 'bitget';
const EXCHANGE_ID = 1;

const config = {
    apiKey: process.env.BITGET_API_KEY,
    secretKey: process.env.BITGET_SECRET_KEY,
    passphrase: process.env.BITGET_PASSPHRASE,
};

const bitgetApi = new EasyAPI(config);

async function fetchAndProcessCommissions() {
    logger.info(`[${EXCHANGE_NAME}] Starting commission data sync cycle...`);

    const IS_TEST_MODE = process.env.MODE === 'test';
    const nowTimestamp = Date.now();
    let startTime;

    // --- 核心修改 ---
    if (IS_TEST_MODE) {
        // 在测试模式下，强制将开始时间设置为1天前
        startTime = nowTimestamp - (2 * 24 * 60 * 60 * 1000);
        logger.info(`[TEST MODE] Fetching data for the last 1 day.`);
    } else {
        // 在正常模式下，使用 stateService 来获取上次同步的时间
        startTime = stateService.getLastSyncTimestamp(EXCHANGE_IDENTIFIER);
    }
    
    if (startTime >= nowTimestamp) {
        logger.info(`[${EXCHANGE_NAME}] No new time window to sync. Skipping cycle.`);
        return;
    }

    try {
        const endpoint = "/api/broker/v1/agent/customer-commissions";
        const params = {
            startTime: startTime.toString(),
            endTime: nowTimestamp.toString(),
            limit: 1000
        };

        logger.info(`[${EXCHANGE_NAME}] Fetching data from ${new Date(startTime).toISOString()} to ${new Date(nowTimestamp).toISOString()}`);
        const response = await bitgetApi.get(endpoint, params);

        // if (IS_TEST_MODE) {
        //     logger.info(`[${EXCHANGE_NAME}] Raw API Response:\n${JSON.stringify(response, null, 2)}`);
        // }

        if (response.code === "00000") {
            const commissionList = response.data.commissionList || [];
            logger.info(`[${EXCHANGE_NAME}] API returned ${commissionList.length} records.`);

            if (commissionList.length > 0) {
                commissionList.sort((a, b) => parseInt(a.date) - parseInt(b.date));
                for (const item of commissionList) {
                    // 将 item.fee 转换为数字进行判断
                    const fee = parseFloat(item.fee);
                    
                    // 只有当手续费大于0时，才处理并推送这条数据
                    if (fee > 0) {
                        await backendApiService.sendTradeData({
                            exchangeUid: item.uid,
                            exchangeId: EXCHANGE_ID,
                            tradeVolumeUsdt: item.dealAmount,
                            feeUsdt: item.fee,
                            tradeDate: moment(parseInt(item.date)).format('YYYY-MM-DD'),
                        }, EXCHANGE_NAME);
                    }
                }
            }
        } else {
            logger.error(`[${EXCHANGE_NAME}] API request failed: ${response.msg}`);
        }
        
        // --- 核心修改 ---
        // 仅在正常模式下更新时间戳，确保测试模式每次都拉取完整7天
        if (!IS_TEST_MODE) {
            stateService.updateLastSyncTimestamp(EXCHANGE_IDENTIFIER, nowTimestamp);
        }

    } catch (error) {
        logger.error(`[${EXCHANGE_NAME}] Failed to fetch commissions: ${error.message}`);
    }
}

function start() {
    fetchAndProcessCommissions();
    setInterval(fetchAndProcessCommissions, 30000);
}

module.exports = {
    start
};