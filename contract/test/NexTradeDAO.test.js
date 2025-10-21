const { expect } = require("chai");
const { ethers } = require("hardhat");
const { time } = require("@nomicfoundation/hardhat-network-helpers");

describe("NexTradeDAO Token 测试", function () {
  let token;
  let owner, team, privateAddr, community, user1, user2;
  let projectAddresses;

  // 常量
  const TOKEN_UNIT = ethers.parseEther("1");
  const TOTAL_SUPPLY = ethers.parseEther("3000000000"); // 30亿
  const SECONDS_PER_DAY = 86400;

  before(async function () {
    // 获取测试账户
    [owner, team, privateAddr, community, user1, user2, ...accounts] = await ethers.getSigners();
    
    // 创建100个项目地址（用测试账户的地址填充）
    projectAddresses = [];
    for (let i = 0; i < 100; i++) {
      // 使用固定地址模式（类似你在 Remix 中使用的）
      const addr = "0x" + (i + 1).toString(16).padStart(40, "0");
      projectAddresses.push(addr);
    }
    
    console.log("\n📝 测试环境准备完成");
    console.log(`Owner: ${owner.address}`);
    console.log(`Team: ${team.address}`);
    console.log(`Private: ${privateAddr.address}`);
    console.log(`Community: ${community.address}`);
  });

  describe("部署测试", function () {
    it("应该成功部署合约（initialDay = 0）", async function () {
      const NexTradeDAO = await ethers.getContractFactory("NexTradeDAO");
      token = await NexTradeDAO.deploy(
        0, // initialDay
        projectAddresses,
        team.address,
        privateAddr.address,
        community.address,
        owner.address
      );

      await token.waitForDeployment();
      console.log(`✅ 合约部署成功: ${await token.getAddress()}`);
    });

    it("应该正确设置基本信息", async function () {
      expect(await token.name()).to.equal("NexTrade DAO");
      expect(await token.symbol()).to.equal("NTX");
      expect(await token.decimals()).to.equal(18);
      expect(await token.owner()).to.equal(owner.address);
      expect(await token.cap()).to.equal(TOTAL_SUPPLY);
      
      console.log("✅ 基本信息验证通过");
    });

    it("应该正确设置项目地址", async function () {
      // 检查前几个和后几个地址
      expect(await token.projectAddresses(0)).to.equal(projectAddresses[0]);
      expect(await token.projectAddresses(99)).to.equal(projectAddresses[99]);
      expect(await token.projectAddresses(100)).to.equal(team.address);
      expect(await token.projectAddresses(101)).to.equal(privateAddr.address);
      expect(await token.projectAddresses(102)).to.equal(community.address);
      
      console.log("✅ 项目地址设置正确");
    });

    it("应该铸造第0天的代币", async function () {
      const totalSupply = await token.totalSupply();
      expect(totalSupply).to.be.gt(0);
      
      // 第0天的发行量
      const dayZeroIssuance = await token.getDailyIssuance(0);
      console.log(`\n📊 第0天铸币详情:`);
      console.log(`   发行量: ${ethers.formatEther(dayZeroIssuance)} NTX`);
      console.log(`   当前总供应: ${ethers.formatEther(totalSupply)} NTX`);
      console.log(`   占总上限比例: ${(Number(ethers.formatEther(totalSupply)) / 3000000000 * 100).toFixed(6)}%`);
      
      // Owner 应该得到 10%
      const ownerBalance = await token.balanceOf(owner.address);
      const expectedOwnerShare = dayZeroIssuance * BigInt(10) / BigInt(100);
      console.log(`\n💰 Owner 分配:`);
      console.log(`   实际余额: ${ethers.formatEther(ownerBalance)} NTX`);
      console.log(`   预期份额 (10%): ${ethers.formatEther(expectedOwnerShare)} NTX`);
      console.log(`   占总铸币比例: ${(Number(ethers.formatEther(ownerBalance)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(2)}%`);
      
      // 统计100个项目地址的分配
      let totalProjectAllocation = BigInt(0);
      let nonZeroProjects = 0;
      let minAllocation = BigInt(2) ** BigInt(256) - BigInt(1); // 最大值
      let maxAllocation = BigInt(0);
      
      for (let i = 0; i < 100; i++) {
        const balance = await token.balanceOf(projectAddresses[i]);
        if (balance > 0) {
          nonZeroProjects++;
          totalProjectAllocation += balance;
          if (balance < minAllocation) minAllocation = balance;
          if (balance > maxAllocation) maxAllocation = balance;
        }
      }
      
      console.log(`\n🏢 100个项目地址分配统计:`);
      console.log(`   总分配: ${ethers.formatEther(totalProjectAllocation)} NTX`);
      console.log(`   非零地址数: ${nonZeroProjects}/100`);
      console.log(`   平均分配: ${ethers.formatEther(totalProjectAllocation / BigInt(100))} NTX`);
      console.log(`   最小分配: ${ethers.formatEther(minAllocation)} NTX`);
      console.log(`   最大分配: ${ethers.formatEther(maxAllocation)} NTX`);
      console.log(`   占总铸币比例: ${(Number(ethers.formatEther(totalProjectAllocation)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(2)}%`);
      
      expect(ownerBalance).to.be.gt(0);
      expect(totalProjectAllocation).to.be.gt(0);
    });
  });

  describe("每日发行量测试", function () {
    it("getDailyIssuance 应该返回正确的值", async function () {
      const day0 = await token.getDailyIssuance(0);
      const day1 = await token.getDailyIssuance(1);
      const day100 = await token.getDailyIssuance(100);
      const day365 = await token.getDailyIssuance(365);
      const day3650 = await token.getDailyIssuance(3650); // 第10年
      const day7299 = await token.getDailyIssuance(7299); // 第20年最后一天
      const day7300 = await token.getDailyIssuance(7300); // 第21年第一天
      const day10000 = await token.getDailyIssuance(10000);
      const day18249 = await token.getDailyIssuance(18249); // 最后一天
      const day18250 = await token.getDailyIssuance(18250); // 超出范围
      
      console.log(`\n📈 每日发行量详细分析:`);
      console.log(`\n🔹 阶段1 (0-20年，线性递减):`);
      console.log(`   Day 0:     ${ethers.formatEther(day0).padStart(25)} NTX`);
      console.log(`   Day 1:     ${ethers.formatEther(day1).padStart(25)} NTX (递减: ${ethers.formatEther(day0 - day1)} NTX)`);
      console.log(`   Day 100:   ${ethers.formatEther(day100).padStart(25)} NTX`);
      console.log(`   Day 365:   ${ethers.formatEther(day365).padStart(25)} NTX (第1年结束)`);
      console.log(`   Day 3650:  ${ethers.formatEther(day3650).padStart(25)} NTX (第10年结束)`);
      console.log(`   Day 7299:  ${ethers.formatEther(day7299).padStart(25)} NTX (阶段1最后)`);
      
      console.log(`\n🔹 阶段2 (21-50年，线性递减至0):`);
      console.log(`   Day 7300:  ${ethers.formatEther(day7300).padStart(25)} NTX (阶段2开始)`);
      console.log(`   Day 10000: ${ethers.formatEther(day10000).padStart(25)} NTX`);
      console.log(`   Day 18249: ${ethers.formatEther(day18249).padStart(25)} NTX (最后一天)`);
      console.log(`   Day 18250: ${ethers.formatEther(day18250).padStart(25)} NTX (超出范围)`);
      
      // 计算每个阶段的递减率
      const phase1DailyDecrease = (day0 - day7299) / BigInt(7299);
      const phase2DailyDecrease = (day7300 - day18249) / BigInt(10949);
      
      console.log(`\n📉 递减率分析:`);
      console.log(`   阶段1平均每日递减: ${ethers.formatEther(phase1DailyDecrease)} NTX/天`);
      console.log(`   阶段2平均每日递减: ${ethers.formatEther(phase2DailyDecrease)} NTX/天`);
      console.log(`   阶段1到阶段2跳跃: ${ethers.formatEther(day7299 - day7300)} NTX (${((Number(ethers.formatEther(day7299 - day7300)) / Number(ethers.formatEther(day7299))) * 100).toFixed(2)}%)`);
      
      // 验证递减
      expect(day0).to.be.gt(day1);
      expect(day7299).to.be.gt(day7300);
      expect(day18249).to.be.gt(0);
      expect(day18250).to.equal(0);
    });
  });

  describe("转账和自动铸币测试", function () {
    it("转账应该触发自动铸币", async function () {
      const beforeSupply = await token.totalSupply();
      const beforeBalance = await token.balanceOf(owner.address);
      const beforeDay = await token.lastMintDay();
      
      console.log(`\n⏰ 转账触发自动铸币测试:`);
      console.log(`   推进前状态:`);
      console.log(`   - 总供应量: ${ethers.formatEther(beforeSupply)} NTX`);
      console.log(`   - Owner余额: ${ethers.formatEther(beforeBalance)} NTX`);
      console.log(`   - 最后铸币天: ${beforeDay.toString()}`);
      
      // 推进时间1天
      await time.increase(SECONDS_PER_DAY);
      
      // 进行一笔转账以触发 _update
      const transferAmount = ethers.parseEther("1");
      await token.transfer(user1.address, transferAmount);
      
      const afterSupply = await token.totalSupply();
      const afterBalance = await token.balanceOf(owner.address);
      const afterDay = await token.lastMintDay();
      const user1Balance = await token.balanceOf(user1.address);
      
      const mintedAmount = afterSupply - beforeSupply;
      const day1Issuance = await token.getDailyIssuance(1);
      
      console.log(`\n   推进1天后状态:`);
      console.log(`   - 总供应量: ${ethers.formatEther(afterSupply)} NTX`);
      console.log(`   - 新铸造量: ${ethers.formatEther(mintedAmount)} NTX`);
      console.log(`   - 理论第1天发行: ${ethers.formatEther(day1Issuance)} NTX`);
      console.log(`   - Owner余额: ${ethers.formatEther(afterBalance)} NTX`);
      console.log(`   - User1收到: ${ethers.formatEther(user1Balance)} NTX`);
      console.log(`   - 最后铸币天: ${afterDay.toString()}`);
      console.log(`   - 铸币准确度: ${(Number(ethers.formatEther(mintedAmount)) / Number(ethers.formatEther(day1Issuance)) * 100).toFixed(6)}%`);
      
      // 总供应量应该增加（铸造了新的一天的代币）
      expect(afterSupply).to.be.gt(beforeSupply);
      
      // User1 应该收到转账
      expect(user1Balance).to.equal(transferAmount);
    });

    it("连续推进多天应该正确铸币", async function () {
      const beforeSupply = await token.totalSupply();
      const beforeDay = await token.lastMintDay();
      
      console.log(`\n⏰ 连续推进多天测试:`);
      console.log(`   推进前:`);
      console.log(`   - 总供应量: ${ethers.formatEther(beforeSupply)} NTX`);
      console.log(`   - 最后铸币天: ${beforeDay.toString()}`);
      
      // 推进5天
      const daysToAdvance = 5;
      await time.increase(daysToAdvance * SECONDS_PER_DAY);
      
      // 触发铸币
      await token.transfer(user2.address, ethers.parseEther("1"));
      
      const afterSupply = await token.totalSupply();
      const afterDay = await token.lastMintDay();
      const mintedAmount = afterSupply - beforeSupply;
      
      // 计算理论铸币量
      let theoreticalMint = BigInt(0);
      for (let i = 0; i < daysToAdvance; i++) {
        const dayNum = Number(beforeDay) + i + 1;
        const issuance = await token.getDailyIssuance(dayNum);
        theoreticalMint += issuance;
        console.log(`   - 第${dayNum}天发行: ${ethers.formatEther(issuance)} NTX`);
      }
      
      console.log(`\n   推进${daysToAdvance}天后:`);
      console.log(`   - 总供应量: ${ethers.formatEther(afterSupply)} NTX`);
      console.log(`   - 实际铸造: ${ethers.formatEther(mintedAmount)} NTX`);
      console.log(`   - 理论铸造: ${ethers.formatEther(theoreticalMint)} NTX`);
      console.log(`   - 最后铸币天: ${afterDay.toString()}`);
      console.log(`   - 铸币准确度: ${(Number(ethers.formatEther(mintedAmount)) / Number(ethers.formatEther(theoreticalMint)) * 100).toFixed(6)}%`);
      console.log(`   - 平均每天: ${ethers.formatEther(mintedAmount / BigInt(daysToAdvance))} NTX`);
      
      expect(afterSupply).to.be.gt(beforeSupply);
      expect(Number(afterDay)).to.equal(Number(beforeDay) + daysToAdvance);
    });
  });

  describe("Vesting 测试", function () {
    it("推进30天应该触发第一个月的 vesting", async function () {
      const beforeTeam = await token.balanceOf(team.address);
      const beforePrivate = await token.balanceOf(privateAddr.address);
      const beforeCommunity = await token.balanceOf(community.address);
      const beforeMonth = await token.lastVestingMonth();
      const beforeSupply = await token.totalSupply();
      
      console.log(`\n💼 Vesting 详细测试 (推进30天):`);
      console.log(`\n   推进前状态:`);
      console.log(`   - Team余额: ${ethers.formatEther(beforeTeam)} NTX`);
      console.log(`   - Private余额: ${ethers.formatEther(beforePrivate)} NTX`);
      console.log(`   - Community余额: ${ethers.formatEther(beforeCommunity)} NTX`);
      console.log(`   - 最后vesting月: ${beforeMonth.toString()}`);
      console.log(`   - 总供应量: ${ethers.formatEther(beforeSupply)} NTX`);
      
      // 推进30天（1个月）
      await time.increase(30 * SECONDS_PER_DAY);
      
      // 触发 vesting
      await token.transfer(user1.address, ethers.parseEther("0.1"));
      
      const afterTeam = await token.balanceOf(team.address);
      const afterPrivate = await token.balanceOf(privateAddr.address);
      const afterCommunity = await token.balanceOf(community.address);
      const afterMonth = await token.lastVestingMonth();
      const afterSupply = await token.totalSupply();
      
      const teamVest = afterTeam - beforeTeam;
      const privateVest = afterPrivate - beforePrivate;
      const communityVest = afterCommunity - beforeCommunity;
      const totalVest = teamVest + privateVest + communityVest;
      
      // 计算理论值
      const TOTAL_SUPPLY = ethers.parseEther("3000000000");
      const teamMonthly = (TOTAL_SUPPLY * BigInt(15) / BigInt(100)) / BigInt(240);
      const privateMonthly = (TOTAL_SUPPLY * BigInt(10) / BigInt(100)) / BigInt(120);
      const communityMonthly = (TOTAL_SUPPLY * BigInt(5) / BigInt(100)) / BigInt(240);
      
      console.log(`\n   推进30天后状态:`);
      console.log(`   - 最后vesting月: ${afterMonth.toString()}`);
      console.log(`\n   🏦 Team (15%, 240个月):`);
      console.log(`   - 实际获得: ${ethers.formatEther(teamVest)} NTX`);
      console.log(`   - 理论每月: ${ethers.formatEther(teamMonthly)} NTX`);
      console.log(`   - 准确度: ${(Number(ethers.formatEther(teamVest)) / Number(ethers.formatEther(teamMonthly)) * 100).toFixed(6)}%`);
      console.log(`   - 当前总额: ${ethers.formatEther(afterTeam)} NTX`);
      
      console.log(`\n   🏦 Private (10%, 120个月):`);
      console.log(`   - 实际获得: ${ethers.formatEther(privateVest)} NTX`);
      console.log(`   - 理论每月: ${ethers.formatEther(privateMonthly)} NTX`);
      console.log(`   - 准确度: ${(Number(ethers.formatEther(privateVest)) / Number(ethers.formatEther(privateMonthly)) * 100).toFixed(6)}%`);
      console.log(`   - 当前总额: ${ethers.formatEther(afterPrivate)} NTX`);
      
      console.log(`\n   🏦 Community (5%, 240个月):`);
      console.log(`   - 实际获得: ${ethers.formatEther(communityVest)} NTX`);
      console.log(`   - 理论每月: ${ethers.formatEther(communityMonthly)} NTX`);
      console.log(`   - 准确度: ${(Number(ethers.formatEther(communityVest)) / Number(ethers.formatEther(communityMonthly)) * 100).toFixed(6)}%`);
      console.log(`   - 当前总额: ${ethers.formatEther(afterCommunity)} NTX`);
      
      console.log(`\n   📊 Vesting总览:`);
      console.log(`   - 总vesting: ${ethers.formatEther(totalVest)} NTX`);
      console.log(`   - 占总供应比: ${(Number(ethers.formatEther(totalVest)) / Number(ethers.formatEther(afterSupply)) * 100).toFixed(4)}%`);
      console.log(`   - 供应量增长: ${ethers.formatEther(afterSupply - beforeSupply)} NTX`);
      
      // 所有三个地址都应该收到 vesting
      expect(afterTeam).to.be.gt(beforeTeam);
      expect(afterPrivate).to.be.gt(beforePrivate);
      expect(afterCommunity).to.be.gt(beforeCommunity);
    });
  });

  describe("销毁代币测试", function () {
    it("应该能够销毁代币", async function () {
      const beforeSupply = await token.totalSupply();
      const beforeBalance = await token.balanceOf(owner.address);
      
      const burnAmount = ethers.parseEther("1000");
      await token.burn(burnAmount);
      
      const afterSupply = await token.totalSupply();
      const afterBalance = await token.balanceOf(owner.address);
      
      console.log(`\n🔥 销毁测试:`);
      console.log(`销毁数量: ${ethers.formatEther(burnAmount)} NTX`);
      console.log(`总供应量减少: ${ethers.formatEther(beforeSupply - afterSupply)} NTX`);
      
      expect(afterSupply).to.equal(beforeSupply - burnAmount);
      expect(afterBalance).to.equal(beforeBalance - burnAmount);
    });
  });

  describe("管理员权限测试", function () {
    it("只有 owner 可以更新项目地址", async function () {
      const newProjectAddresses = [...projectAddresses];
      newProjectAddresses[0] = user1.address;
      
      // Owner 应该可以更新
      await expect(
        token.setProjectAddresses(
          newProjectAddresses,
          team.address,
          privateAddr.address,
          community.address
        )
      ).to.not.be.reverted;
      
      expect(await token.projectAddresses(0)).to.equal(user1.address);
      
      // 非 owner 不能更新
      await expect(
        token.connect(user1).setProjectAddresses(
          newProjectAddresses,
          team.address,
          privateAddr.address,
          community.address
        )
      ).to.be.reverted;
      
      console.log("✅ 权限控制测试通过");
    });

    it("可以转移所有权", async function () {
      await token.transferOwnership(user1.address);
      expect(await token.owner()).to.equal(user1.address);
      
      // 转回来
      await token.connect(user1).transferOwnership(owner.address);
      expect(await token.owner()).to.equal(owner.address);
      
      console.log("✅ 所有权转移测试通过");
    });
  });

  describe("上限测试", function () {
    it("总供应量不应超过上限", async function () {
      const totalSupply = await token.totalSupply();
      const cap = await token.cap();
      
      console.log(`\n📊 供应量检查:`);
      console.log(`当前总供应: ${ethers.formatEther(totalSupply)} NTX`);
      console.log(`最大上限: ${ethers.formatEther(cap)} NTX`);
      console.log(`剩余可铸造: ${ethers.formatEther(cap - totalSupply)} NTX`);
      console.log(`已使用比例: ${(Number(ethers.formatEther(totalSupply)) / Number(ethers.formatEther(cap)) * 100).toFixed(8)}%`);
      
      expect(totalSupply).to.be.lte(cap);
    });

    it("完整的50年铸币量预测", async function () {
      console.log(`\n🔮 50年完整铸币预测:`);
      
      // 阶段1: 前20年 (7300天)
      let phase1Total = BigInt(0);
      for (let i = 0; i < 7300; i++) {
        phase1Total += await token.getDailyIssuance(i);
      }
      
      // 阶段2: 21-50年 (10950天)
      let phase2Total = BigInt(0);
      for (let i = 7300; i < 18250; i++) {
        phase2Total += await token.getDailyIssuance(i);
      }
      
      const totalMining = phase1Total + phase2Total;
      const cap = await token.cap();
      
      // Vesting总量
      const teamTotal = cap * BigInt(15) / BigInt(100);
      const privateTotal = cap * BigInt(10) / BigInt(100);
      const communityTotal = cap * BigInt(5) / BigInt(100);
      const vestingTotal = teamTotal + privateTotal + communityTotal;
      
      const grandTotal = totalMining + vestingTotal;
      
      console.log(`\n   ⛏️  挖矿部分 (70%):`);
      console.log(`   - 阶段1 (0-20年): ${ethers.formatEther(phase1Total).padStart(25)} NTX`);
      console.log(`   - 阶段2 (21-50年): ${ethers.formatEther(phase2Total).padStart(25)} NTX`);
      console.log(`   - 挖矿总计: ${ethers.formatEther(totalMining).padStart(25)} NTX`);
      console.log(`   - 占比: ${(Number(ethers.formatEther(totalMining)) / Number(ethers.formatEther(cap)) * 100).toFixed(2)}%`);
      
      console.log(`\n   🔓 Vesting部分 (30%):`);
      console.log(`   - Team (15%, 240月): ${ethers.formatEther(teamTotal).padStart(25)} NTX`);
      console.log(`   - Private (10%, 120月): ${ethers.formatEther(privateTotal).padStart(25)} NTX`);
      console.log(`   - Community (5%, 240月): ${ethers.formatEther(communityTotal).padStart(25)} NTX`);
      console.log(`   - Vesting总计: ${ethers.formatEther(vestingTotal).padStart(25)} NTX`);
      console.log(`   - 占比: ${(Number(ethers.formatEther(vestingTotal)) / Number(ethers.formatEther(cap)) * 100).toFixed(2)}%`);
      
      console.log(`\n   📈 50年总计:`);
      console.log(`   - 预计总发行: ${ethers.formatEther(grandTotal).padStart(25)} NTX`);
      console.log(`   - 合约上限: ${ethers.formatEther(cap).padStart(25)} NTX`);
      console.log(`   - 差异: ${ethers.formatEther(cap - grandTotal).padStart(25)} NTX`);
      console.log(`   - 准确度: ${(Number(ethers.formatEther(grandTotal)) / Number(ethers.formatEther(cap)) * 100).toFixed(8)}%`);
      
      // 验证总量接近30亿（允许小误差，约0.006%）
      const tolerance = ethers.parseEther("200000"); // 20万 NTX误差容忍 (约0.0067%)
      expect(grandTotal).to.be.closeTo(cap, tolerance);
      
      if (grandTotal > cap) {
        console.log(`\n   ⚠️  注意: 理论总量略超上限 ${ethers.formatEther(grandTotal - cap)} NTX`);
        console.log(`   这是由于线性递减公式的舍入误差造成的，实际部署时会被cap()限制`);
      }
    });
  });

  describe("InitialDay = 10 部署测试", function () {
    it("使用 initialDay = 10 部署应该成功", async function () {
      const NexTradeDAO = await ethers.getContractFactory("NexTradeDAO");
      const token10 = await NexTradeDAO.deploy(
        10, // initialDay
        projectAddresses,
        team.address,
        privateAddr.address,
        community.address,
        owner.address
      );

      await token10.waitForDeployment();
      
      console.log(`\n✅ initialDay=10 部署详细分析:`);
      
      // 检查初始状态
      const totalSupply = await token10.totalSupply();
      const lastMintDay = await token10.lastMintDay();
      const lastVestingMonth = await token10.lastVestingMonth();
      
      console.log(`\n   基础信息:`);
      console.log(`   - 合约地址: ${await token10.getAddress()}`);
      console.log(`   - 初始天数: 10`);
      console.log(`   - 最后铸币天: ${lastMintDay.toString()}`);
      console.log(`   - 最后vesting月: ${lastVestingMonth.toString()}`);
      
      // 计算理论铸币量
      let theoreticalTotal = BigInt(0);
      console.log(`\n   📊 前10天发行量明细:`);
      for (let i = 0; i < 10; i++) {
        const dayIssuance = await token10.getDailyIssuance(i);
        theoreticalTotal += dayIssuance;
        console.log(`   Day ${i}: ${ethers.formatEther(dayIssuance).padStart(25)} NTX`);
      }
      
      console.log(`\n   💰 供应量统计:`);
      console.log(`   - 实际总供应: ${ethers.formatEther(totalSupply)} NTX`);
      console.log(`   - 理论铸造量: ${ethers.formatEther(theoreticalTotal)} NTX`);
      console.log(`   - 准确度: ${(Number(ethers.formatEther(totalSupply)) / Number(ethers.formatEther(theoreticalTotal)) * 100).toFixed(6)}%`);
      console.log(`   - 占30亿上限: ${(Number(ethers.formatEther(totalSupply)) / 3000000000 * 100).toFixed(6)}%`);
      
      // 检查各方余额
      const ownerBal = await token10.balanceOf(owner.address);
      const teamBal = await token10.balanceOf(team.address);
      const privateBal = await token10.balanceOf(privateAddr.address);
      const communityBal = await token10.balanceOf(community.address);
      
      // 统计项目地址
      let projectTotal = BigInt(0);
      for (let i = 0; i < 100; i++) {
        const bal = await token10.balanceOf(projectAddresses[i]);
        projectTotal += bal;
      }
      
      console.log(`\n   🏢 余额分配:`);
      console.log(`   - Owner: ${ethers.formatEther(ownerBal).padStart(25)} NTX (${(Number(ethers.formatEther(ownerBal)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(2)}%)`);
      console.log(`   - 100个项目: ${ethers.formatEther(projectTotal).padStart(25)} NTX (${(Number(ethers.formatEther(projectTotal)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(2)}%)`);
      console.log(`   - Team: ${ethers.formatEther(teamBal).padStart(25)} NTX (${(Number(ethers.formatEther(teamBal)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(2)}%)`);
      console.log(`   - Private: ${ethers.formatEther(privateBal).padStart(25)} NTX (${(Number(ethers.formatEther(privateBal)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(2)}%)`);
      console.log(`   - Community: ${ethers.formatEther(communityBal).padStart(25)} NTX (${(Number(ethers.formatEther(communityBal)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(2)}%)`);
      
      const accountedFor = ownerBal + projectTotal + teamBal + privateBal + communityBal;
      console.log(`\n   ✅ 余额验证:`);
      console.log(`   - 已分配总额: ${ethers.formatEther(accountedFor)} NTX`);
      console.log(`   - 供应量: ${ethers.formatEther(totalSupply)} NTX`);
      console.log(`   - 差异: ${ethers.formatEther(totalSupply - accountedFor)} NTX`);
      console.log(`   - 完整性: ${(Number(ethers.formatEther(accountedFor)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(8)}%`);
      
      expect(totalSupply).to.be.gt(0);
      expect(lastMintDay).to.equal(9); // 应该是9（从0开始）
    });

    it("详细的逐天余额追踪（前7天）", async function () {
      const NexTradeDAO = await ethers.getContractFactory("NexTradeDAO");
      const trackToken = await NexTradeDAO.deploy(
        0, // initialDay
        projectAddresses,
        team.address,
        privateAddr.address,
        community.address,
        owner.address
      );
      await trackToken.waitForDeployment();

      console.log(`\n📅 逐天余额追踪详细分析（前7天）:`);
      console.log(`\n合约地址: ${await trackToken.getAddress()}`);
      
      // 选择10个项目地址进行追踪
      const trackedProjects = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90];
      
      for (let day = 0; day <= 6; day++) {
        if (day > 0) {
          // 推进1天
          await time.increase(SECONDS_PER_DAY);
          // 触发铸币
          await trackToken.transfer(user1.address, ethers.parseEther("0.001"));
        }
        
        const totalSupply = await trackToken.totalSupply();
        const dayIssuance = await trackToken.getDailyIssuance(day);
        const lastMintDay = await trackToken.lastMintDay();
        
        console.log(`\n${'='.repeat(80)}`);
        console.log(`📆 第 ${day} 天 (lastMintDay: ${lastMintDay})`);
        console.log(`${'='.repeat(80)}`);
        
        // 基础信息
        console.log(`\n📊 发行信息:`);
        console.log(`   当日理论发行: ${ethers.formatEther(dayIssuance).padStart(30)} NTX`);
        console.log(`   累计总供应量: ${ethers.formatEther(totalSupply).padStart(30)} NTX`);
        console.log(`   占上限比例:   ${(Number(ethers.formatEther(totalSupply)) / 3000000000 * 100).toFixed(8).padStart(30)}%`);
        
        // Owner余额
        const ownerBalance = await trackToken.balanceOf(owner.address);
        console.log(`\n👤 Owner (10%份额):`);
        console.log(`   余额: ${ethers.formatEther(ownerBalance).padStart(35)} NTX`);
        console.log(`   占总供应: ${(Number(ethers.formatEther(ownerBalance)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(4).padStart(35)}%`);
        
        // 特殊地址余额
        const teamBalance = await trackToken.balanceOf(team.address);
        const privateBalance = await trackToken.balanceOf(privateAddr.address);
        const communityBalance = await trackToken.balanceOf(community.address);
        
        console.log(`\n🏦 Vesting地址 (30天后才开始):`);
        console.log(`   Team:      ${ethers.formatEther(teamBalance).padStart(35)} NTX`);
        console.log(`   Private:   ${ethers.formatEther(privateBalance).padStart(35)} NTX`);
        console.log(`   Community: ${ethers.formatEther(communityBalance).padStart(35)} NTX`);
        
        // 统计100个项目地址
        let projectTotal = BigInt(0);
        let projectBalances = [];
        for (let i = 0; i < 100; i++) {
          const balance = await trackToken.balanceOf(projectAddresses[i]);
          projectBalances.push({ index: i, balance });
          projectTotal += balance;
        }
        
        console.log(`\n🏢 100个项目地址统计 (90%份额):`);
        console.log(`   总计: ${ethers.formatEther(projectTotal).padStart(40)} NTX`);
        console.log(`   占总供应: ${(Number(ethers.formatEther(projectTotal)) / Number(ethers.formatEther(totalSupply)) * 100).toFixed(4).padStart(40)}%`);
        console.log(`   平均: ${ethers.formatEther(projectTotal / BigInt(100)).padStart(40)} NTX`);
        
        // 找出最大和最小
        projectBalances.sort((a, b) => {
          if (a.balance > b.balance) return -1;
          if (a.balance < b.balance) return 1;
          return 0;
        });
        
        console.log(`\n   Top 3 项目地址:`);
        for (let i = 0; i < 3; i++) {
          const { index, balance } = projectBalances[i];
          const percentage = (Number(ethers.formatEther(balance)) / Number(ethers.formatEther(projectTotal)) * 100).toFixed(4);
          console.log(`   #${(i+1)} 地址[${index}]: ${ethers.formatEther(balance).padStart(30)} NTX (${percentage.padStart(8)}% of projects)`);
        }
        
        console.log(`\n   Bottom 3 项目地址:`);
        for (let i = 97; i < 100; i++) {
          const { index, balance } = projectBalances[i];
          const percentage = (Number(ethers.formatEther(balance)) / Number(ethers.formatEther(projectTotal)) * 100).toFixed(4);
          console.log(`   #${(i+1)} 地址[${index}]: ${ethers.formatEther(balance).padStart(30)} NTX (${percentage.padStart(8)}% of projects)`);
        }
        
        // 展示随机选择的10个地址
        console.log(`\n   随机抽样10个地址详情:`);
        for (const idx of trackedProjects) {
          const projectBal = projectBalances.find(p => p.index === idx);
          if (projectBal) {
            const percentage = (Number(ethers.formatEther(projectBal.balance)) / Number(ethers.formatEther(projectTotal)) * 100).toFixed(4);
            console.log(`   地址[${idx.toString().padStart(2)}]: ${ethers.formatEther(projectBal.balance).padStart(30)} NTX (${percentage.padStart(8)}%)`);
          }
        }
        
        // 验证总和
        const accountedTotal = ownerBalance + projectTotal + teamBalance + privateBalance + communityBalance;
        const difference = totalSupply - accountedTotal;
        
        console.log(`\n✅ 完整性验证:`);
        console.log(`   Owner + Projects + Vesting: ${ethers.formatEther(accountedTotal).padStart(30)} NTX`);
        console.log(`   总供应量: ${ethers.formatEther(totalSupply).padStart(46)} NTX`);
        console.log(`   差异: ${ethers.formatEther(difference).padStart(50)} NTX`);
        console.log(`   准确度: ${((Number(ethers.formatEther(accountedTotal)) / Number(ethers.formatEther(totalSupply))) * 100).toFixed(10).padStart(48)}%`);
        
        // 如果有user1的转账，也显示
        if (day > 0) {
          const user1Bal = await trackToken.balanceOf(user1.address);
          console.log(`   User1转账累计: ${ethers.formatEther(user1Bal).padStart(42)} NTX`);
        }
      }
      
      console.log(`\n${'='.repeat(80)}`);
      console.log(`📊 7天追踪完成`);
      console.log(`${'='.repeat(80)}\n`);
    });
  });
});
