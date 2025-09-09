require('dotenv').config();
const logger = require('./src/utils/logger');
const bitgetService = require('./src/exchanges/bitget');
const xtService = require('./src/exchanges/xt');

logger.info('Starting Exchange Service...');

// 启动 Bitget 服务
bitgetService.start();
logger.info('Bitget service started.');

// 启动 XT.COM 服务
xtService.start();
logger.info('XT.COM service started.');

logger.info('All exchange services are running.');