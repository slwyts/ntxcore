const moment = require('moment');
const GateIO = require('./easyapi');
const backendApiService = require('../../services/backendApiService');
const stateService = require('../../services/stateService');
const logger = require('../../utils/logger');

const EXCHANGE_NAME = 'Gate.io';
const EXCHANGE_IDENTIFIER = 'gate';
const EXCHANGE_ID = 7;

const config = {
    apiKey: process.env.GATE_API_KEY,
    secretKey: process.env.GATE_SECRET_KEY,
};

const gateApi = new GateIO(config);
let tickerCache = {}; // 用于缓存交易对价格

/**
 * 获取所有交易对的最新价格并缓存
 */
async function fetchAndCacheTickers() {
    try {
        logger.info(`[${EXCHANGE_NAME}] Fetching all ticker prices...`);
        const tickers = await gateApi.get('/spot/tickers');
        const newCache = {};
        for (const ticker of tickers) {
            // 我们只关心USDT交易对
            if (ticker.currency_pair.endsWith('_USDT')) {
                newCache[ticker.currency_pair] = parseFloat(ticker.last);
            }
        }
        tickerCache = newCache;
        logger.info(`[${EXCHANGE_NAME}] Ticker cache updated with ${Object.keys(tickerCache).length} USDT pairs.`);
    } catch (error) {
        logger.error(`[${EXCHANGE_NAME}] Failed to fetch and cache tickers: ${error.message}`);
    }
}

/**
 * 将给定资产的数量转换为 USDT
 */
function convertToUsdt(asset, amount) {
    if (asset.toUpperCase() === 'USDT') {
        return parseFloat(amount);
    }

    const pair = `${asset.toUpperCase()}_USDT`;
    const price = tickerCache[pair];

    if (price) {
        return parseFloat(amount) * price;
    }

    logger.warn(`[${EXCHANGE_NAME}] No price found for ${pair} in cache. Cannot convert to USDT.`);
    return 0; // 如果找不到价格，返回0
}


async function fetchAndProcessCommissions() {
    logger.info(`[${EXCHANGE_NAME}] Starting commission data sync cycle...`);

    await fetchAndCacheTickers();
    if (Object.keys(tickerCache).length === 0) {
        logger.error(`[${EXCHANGE_NAME}] Ticker cache is empty. Skipping this cycle to avoid incorrect data.`);
        return;
    }

    const IS_TEST_MODE = process.env.MODE === 'test';
    const now = moment();
    const nowTimestamp = now.unix();
    let startTime;

    if (IS_TEST_MODE) {
        startTime = now.clone().subtract(7, 'days').unix();
        logger.info(`[TEST MODE] Fetching data for the last 7 days.`);
    } else {
        const lastSyncMs = stateService.getLastSyncTimestamp(EXCHANGE_IDENTIFIER);
        startTime = Math.floor(lastSyncMs / 1000);
    }
    
    if (nowTimestamp - startTime > 30 * 24 * 60 * 60) {
        logger.warn(`[${EXCHANGE_NAME}] Time range exceeds 30 days. Adjusting start time.`);
        startTime = nowTimestamp - (30 * 24 * 60 * 60);
    }

    if (startTime >= nowTimestamp) {
        logger.info(`[${EXCHANGE_NAME}] No new time window to sync. Skipping cycle.`);
        return;
    }

    try {
        const endpoint = "/rebate/agency/transaction_history";
        const params = {
            from: startTime,
            to: nowTimestamp,
            limit: 1000
        };

        logger.info(`[${EXCHANGE_NAME}] Fetching data from ${new Date(startTime * 1000).toISOString()} to ${new Date(nowTimestamp * 1000).toISOString()}`);
        const response = await gateApi.get(endpoint, params);

        if (IS_TEST_MODE) {
            logger.info(`[${EXCHANGE_NAME}] Raw API Response:\n${JSON.stringify(response, null, 2)}`);
        }
        
        if (response && typeof response === 'object' && Array.isArray(response.list)) {
            const transactionList = response.list || [];
            logger.info(`[${EXCHANGE_NAME}] API returned ${transactionList.length} records.`);

            if (transactionList.length > 0) {
                transactionList.sort((a, b) => a.transaction_time - b.transaction_time);
                
                for (const item of transactionList) {
                    const fee = parseFloat(item.fee);
                    
                    if (fee > 0) {
                        const tradeVolumeUsdt = convertToUsdt(item.amount_asset, item.amount);
                        const feeUsdt = convertToUsdt(item.fee_asset, item.fee);

                        await backendApiService.sendTradeData({
                            exchangeUid: item.user_id.toString(),
                            exchangeId: EXCHANGE_ID,
                            tradeVolumeUsdt: tradeVolumeUsdt,
                            feeUsdt: feeUsdt,
                            tradeDate: moment.unix(item.transaction_time).format('YYYY-MM-DD'),
                        }, `${EXCHANGE_NAME} - ${item.source}`);
                    }
                }
            }
        } else {
            logger.error(`[${EXCHANGE_NAME}] API request failed or returned unexpected format: ${JSON.stringify(response)}`);
        }
        
        if (!IS_TEST_MODE) {
            stateService.updateLastSyncTimestamp(EXCHANGE_IDENTIFIER, now.valueOf());
        }

    } catch (error) {
        logger.error(`[${EXCHANGE_NAME}] Failed to fetch commissions: ${error.message}`);
    }
}

function start() {
    fetchAndProcessCommissions();
    setInterval(fetchAndProcessCommissions, 60000); // 60秒
}

module.exports = {
    start
};