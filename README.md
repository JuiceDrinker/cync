# Cync 

A terminal-based user interface for hosting files on S3 with seamless file synchronization capabilities.

Supports bi-directional sync without versioning.

## Installation

`cync` is available on [crates.io](https://crates.io/crates/cync) and can be installed by running `cargo install cync`.

## Dependencies
- [AWS CLI](https://docs.aws.amazon.com/cli/latest/userguide/cli-chap-welcome.html)
- Configure [AWS SSO](https://docs.aws.amazon.com/cli/latest/userguide/cli-configure-sso.html)

## Usage
- Run `cync init` to run the setup wizard the first time
- Run `cync` to run CLI thereafter
