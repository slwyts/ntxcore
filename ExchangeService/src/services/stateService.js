const fs = require('fs');
const path = require('path');
const logger = require('../utils/logger');

const stateFilePath = path.join(process.cwd(), 'state.json');

class StateService {
    constructor() {
        this.state = this.loadState();
    }

    loadState() {
        try {
            if (fs.existsSync(stateFilePath)) {
                logger.info('Loading state from state.json...');
                const rawData = fs.readFileSync(stateFilePath);
                return JSON.parse(rawData);
            } else {
                logger.warn('state.json not found. Initializing new state.');
                return {};
            }
        } catch (error) {
            logger.error(`Failed to load state.json: ${error.message}. Starting with a fresh state.`);
            return {};
        }
    }

    saveState() {
        try {
            fs.writeFileSync(stateFilePath, JSON.stringify(this.state, null, 2));
        } catch (error) {
            logger.error(`Failed to save state to state.json: ${error.message}`);
        }
    }

    getLastSyncTimestamp(exchangeName) {
        if (this.state[exchangeName] && this.state[exchangeName].lastSyncTimestamp) {
            return this.state[exchangeName].lastSyncTimestamp;
        }
        
        // --- 核心修改 ---
        // 修正了用于计算10分钟前的数字
        const tenMinutesAgo = Date.now() - (10 * 60 * 1000);
        logger.info(`[${exchangeName}] No last sync timestamp found. Setting to 10 minutes ago for initial sync.`);
        return tenMinutesAgo;
    }

    updateLastSyncTimestamp(exchangeName, timestamp) {
        if (!this.state[exchangeName]) {
            this.state[exchangeName] = {};
        }
        if (!this.state[exchangeName].lastSyncTimestamp || timestamp > this.state[exchangeName].lastSyncTimestamp) {
            this.state[exchangeName].lastSyncTimestamp = timestamp;
            logger.info(`[${exchangeName}] Updating last sync timestamp to: ${new Date(timestamp).toISOString()}`);
            this.saveState();
        }
    }
}

module.exports = new StateService();