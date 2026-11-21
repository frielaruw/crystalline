'use strict';

function regOp(hive, subkey, name, valueType, applyValue, defaultValue = 'ABSENT') {
  return {
    kind: 'registry',
    hive,
    subkey,
    name,
    valueType,
    applyValue: String(applyValue),
    defaultValue: String(defaultValue),
  };
}

function svcOp(serviceName, applyStart, defaultStart) {
  return {
    kind: 'service',
    serviceName,
    applyValue: String(applyStart),
    defaultValue: String(defaultStart),
  };
}

function taskOp(taskPath, applyState = 'Disabled', defaultState = 'Enabled') {
  return {
    kind: 'schtask',
    taskPath,
    applyValue: applyState,
    defaultValue: defaultState,
  };
}

function powerOp(subgroupGuid, settingGuid, applyValue, defaultValue, applyCmd = '', revertCmd = '') {
  return {
    kind: 'powercfg',
    subgroupGuid,
    settingGuid,
    applyValue: String(applyValue),
    defaultValue: String(defaultValue),
    applyCmd,
    revertCmd,
  };
}

function netshTcpOp(paramKey, applyValue, defaultValue, applyCmd, revertCmd) {
  return {
    kind: 'netsh_tcp',
    paramKey,
    applyValue: String(applyValue),
    defaultValue: String(defaultValue),
    applyCmd,
    revertCmd,
  };
}

function makeTweak({
  id,
  title,
  padLabel,
  category,
  subcategory,
  description,
  technicalNotes,
  safetyNotes = 'Captured to SHA-256 Snapshot Vault + offline WinRE .reg rollback before apply.',
  risk = 'safe',
  rebootRequired = false,
  explorerRestart = false,
  requiresAdmin = true,
  tags = [],
  eligibility = {},
  conflictsWith = [],
  nearMatchRule = null,
  ops = [],
}) {
  return {
    id,
    title,
    padLabel: padLabel || title.slice(0, 24),
    category,
    subcategory,
    description,
    technicalNotes,
    safetyNotes,
    risk,
    rebootRequired,
    explorerRestart,
    requiresAdmin,
    tags,
    eligibility: {
      minBuild: eligibility.minBuild ?? null,
      maxBuild: eligibility.maxBuild ?? null,
      minRamGB: eligibility.minRamGB ?? null,
      requiresSSD: Boolean(eligibility.requiresSSD),
      desktopOnly: Boolean(eligibility.desktopOnly),
      warnOnLaptop: Boolean(eligibility.warnOnLaptop),
      warnIfPrinters: Boolean(eligibility.warnIfPrinters),
      warnIfBluetooth: Boolean(eligibility.warnIfBluetooth),
      warnIfXbox: Boolean(eligibility.warnIfXbox),
      warnIfTouchscreen: Boolean(eligibility.warnIfTouchscreen),
    },
    conflictsWith,
    nearMatchRule,
    ops,
  };
}

module.exports = { regOp, svcOp, taskOp, powerOp, netshTcpOp, makeTweak };
