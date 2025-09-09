const moment = require('moment');
const XT = require('./easyapi.js');
const backendApiService = require('../../services/backendApiService');
const stateService = require('../../services/stateService');
const logger = require('../../utils/logger');

const EXCHANGE_NAME = 'XT.COM';
const EXCHANGE_IDENTIFIER = 'xt';
const EXCHANGE_ID = 5;

const config = {
    apiKey: process.env.XT_API_KEY,
    secretKey: process.env.XT_SECRET_KEY,
    spotUrl: process.env.XT_SPOT_API_URL || 'https://sapi.xt.com',
    futureUrl: process.env.XT_FUTURE_API_URL || 'https://fapi.xt.com',
    apiUrl: process.env.XT_API_URL || 'https://api.xt.com'
};

if (!config.apiKey || !config.secretKey) {
    logger.error(`[${EXCHANGE_NAME}] API key and secret key must be configured in .env`);
    process.exit(1);
}

const xtClient = new XT(config);

function calculateOriginalFee(commission, rebateRate) {
    if (rebateRate === 0 || !rebateRate) {
        return 0;
    }
    return commission / rebateRate;
}

async function fetchAndProcessCommissions() {
    logger.info(`[${EXCHANGE_NAME}] Starting commission data sync cycle...`);

    const IS_TEST_MODE = process.env.MODE === 'test';
    const nowTimestamp = Date.now();
    let startTime;

    if (IS_TEST_MODE) {
        startTime = nowTimestamp - (7 * 24 * 60 * 60 * 1000);
        logger.info(`[TEST MODE] Fetching data for the last 7 days.`);
    } else {
        startTime = stateService.getLastSyncTimestamp(EXCHANGE_IDENTIFIER);
    }
    
    if (startTime >= nowTimestamp) {
        logger.info(`[${EXCHANGE_NAME}] No new time window to sync. Skipping cycle.`);
        return;
    }
    
    const endpoint = "/v4/referal/invite/agent/rebate/data";
    const types = [{ id: 1, name: 'Spot' }, { id: 2, name: 'Futures' }];

    try {
        let allNewEntries = [];

        for (const typeInfo of types) {
             const params = {
                startTime: startTime,
                endTime: nowTimestamp,
                type: typeInfo.id,
                inviteCode: "BTEH6V"
            };

            logger.info(`[${EXCHANGE_NAME} - ${typeInfo.name}] Fetching data from ${new Date(startTime).toISOString()} to ${new Date(nowTimestamp).toISOString()}`);
            const response = await xtClient.request({
                apiType: 'uapi', 
                method: 'GET',
                path: endpoint,
                params: params
            });

            if (IS_TEST_MODE) {
                logger.info(`[${EXCHANGE_NAME} - ${typeInfo.name}] Raw API Response:\n${JSON.stringify(response, null, 2)}`);
            }

            if (response && response.rc === 0 && response.mc === "SUCCESS") {
                const items = response.result.items || [];
                logger.info(`[${EXCHANGE_NAME} - ${typeInfo.name}] API returned ${items.length} records.`);
                
                const parsedEntries = items.map(item => {
                    const commission = parseFloat(item.commissionAmount || 0);
                    let rebateRate = 0;
                    
                    // 这里的逻辑就是您期望的“自动识别”
                    if (item.type === 1) { // 现货
                        rebateRate = parseFloat(item.spotRebateRate || 0);
                    } else if (item.type === 2) { // 合约
                        rebateRate = parseFloat(item.futuresRebateRate || 0);
                    }

                    return {
                        rawTimestamp: item.date,
                        date: moment(item.date).format('YYYY-MM-DD'),
                        uid: item.uid,
                        type: item.type,
                        typeName: typeInfo.name,
                        dealAmount: parseFloat(item.totalTradeUsdtAmount || 0),
                        fee: calculateOriginalFee(commission, rebateRate)
                    };
                });
                allNewEntries.push(...parsedEntries);
            } else {
                 logger.error(`[${EXCHANGE_NAME} - ${typeInfo.name}] API request failed: ${JSON.stringify(response)}`);
            }
        }

        if (allNewEntries.length > 0) {
            allNewEntries.sort((a, b) => a.rawTimestamp - b.rawTimestamp);

            for (const entry of allNewEntries) {
                 await backendApiService.sendTradeData({
                     exchangeUid: entry.uid,
                     exchangeId: EXCHANGE_ID,
                     tradeVolumeUsdt: entry.dealAmount,
                     feeUsdt: entry.fee,
                     tradeDate: entry.date,
                 }, `${EXCHANGE_NAME} - ${entry.typeName}`);
            }
        }

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