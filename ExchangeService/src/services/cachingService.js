const logger = require('../utils/logger');
const moment = require('moment');

class CachingService {
    constructor() {
        this.processedEntries = new Set();
        this.scheduleDailyReset();
    }

    /**
     * 为一条交易记录生成唯一的key
     * @param {string} exchangeName - 交易所名称 (e.g., 'bitget', 'xt')
     * @param {object} entry - 交易数据
     * @returns {string} 唯一的key
     */
    _generateKey(exchangeName, entry) {
        // 对于XT，需要考虑type，因为同一个用户同一天可能有现货和合约两条数据
        if (exchangeName.toLowerCase() === 'xt') {
            return `${exchangeName}-${entry.date}-${entry.uid}-${entry.type}`;
        }
        // Bitget 和其他交易所默认使用 date 和 uid
        return `${exchangeName}-${entry.date}-${entry.uid}`;
    }

    /**
     * 检查一个条目是否已经被处理过
     * @param {string} exchangeName 
     * @param {object} entry 
     * @returns {boolean}
     */
    has(exchangeName, entry) {
        const key = this._generateKey(exchangeName, entry);
        return this.processedEntries.has(key);
    }

    /**
     * 将一个条目标记为已处理
     * @param {string} exchangeName 
     * @param {object} entry 
     */
    add(exchangeName, entry) {
        const key = this._generateKey(exchangeName, entry);
        this.processedEntries.add(key);
    }

    /**
     * 安排在每天午夜清空缓存
     */
    scheduleDailyReset() {
        const reset = () => {
            this.processedEntries.clear();
            logger.info('Caching service: Daily cache has been cleared.');
        };

        // 每秒检查一次时间，以精确触发清空操作
        setInterval(() => {
            const now = moment();
            if (now.hour() === 0 && now.minute() === 0 && now.second() === 0) {
                reset();
            }
        }, 1000);
    }
}

// 使用单例模式，确保整个应用只有一个缓存实例
module.exports = new CachingService();