# Changelog

All notable changes to taskit-init will be documented in this file.

## Unreleased

### Features

- Add mdBook scaffold generator (42ee04e)
- Expand scaffolding with hooks, CI, deny.toml, .ctx/, and smart
  discovery (c6cd574)

### Refactoring

- Remove xtask crate generation; use cargo alias instead

## [0.6.0] - 2026-06-28

### Features

- Integrate output formatters and cargo alias on init (9dba361, a2c1d65)

### Fixes

- Empty CiConfig.steps runs nothing (d21e831)
