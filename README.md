# TeleGPT

[API Docs](https://icystudio.github.io/TeleGPT/telegpt_core) | [Releases](https://github.com/IcyStudio/TeleGPT/releases) | [Twitter](https://twitter.com/unixzii)

![Hero](./artworks/hero.png)

An out-of-box ChatGPT bot for Telegram.

TeleGPT is a Telegram bot based on [**teloxide**](https://github.com/teloxide/teloxide) framework and [**async_openai**](https://github.com/64bit/async-openai). It provides an easy way to interact with the latest ChatGPT models utilizing your own API key.

## Features

🦀 **Lightning fast** with pure Rust codebase.<br>
📢 **All types of chat** (private and group) supports.<br>
🚀 **Live streaming tokens** to your message bubble.<br>
💸 **Token usage** statistic recording and queryable via commands.<br>
⚙️ **Fully customizable** with file-based configuration.<br>
✋ **Admin features** (Beta) and user access control supports.

## Getting TeleGPT

### Download from release

We recommend you to download the pre-built binary directly from the [releases](https://github.com/IcyStudio/TeleGPT/releases) page. Currently, Linux and macOS (Intel and Apple Silicon) hosts are supported.

### Build from source

Clone the repository and run:

```shell
$ cargo build --release
```

## Usage

You need to create a configuration file before running the bot. The program reads `telegpt.config.json` from your current working directory by default, and you can also specify the config file path via `-c` option.

The configuration is described in this [doc](https://icystudio.github.io/TeleGPT/telegpt_core/config/), and here is an example:

```json
{
  "openaiAPIKey": "sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "botToken": "8888888888:XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
  "adminUsernames": ["cyandev"],
  "conversationLimit": 30,
  "databasePath": "./path/to/telegpt.sqlite",
  "i18n": {
    "resetPrompt": "I’m ready for a new challenge. What can I do for you now?"
  }
}
```

To start the bot, simply run:

```shell
$ /path/to/telegpt
```

When you see the message `Bot is started`, you are ready to go!

### Enable the verbose logging

> **Note:** Users' input will be logged in `DEBUG` level. To protect user privacy, please don't enable it in the production environment.

For debugging purpose, you can enable the verbose logs by setting `RUST_LOG` environment variable. For example:

```shell
$ RUST_LOG=TRACE /path/to/telegpt
```

### Admin Features (Beta)

> This feature depends on database to store the configurations. To ensure your data will not be lost after relaunching, you need to set a database path in the config file.

The bot has some basic admin features built-in. You can control who are allowed to use the bot, and dynamically change the member list via a set of commands.

By default, the bot is available for public use. It means everybody who adds it can chat with it, which may heavily cost your tokens. If you want to deploy and use the bot only within a small group of people, send `/set_public off` command to make the bot private. When you want to make it public again, send `/set_public on`.

When the bot is in private mode, only admin users and invited members can chat with it. You can add or delete members via `/add_member` and `/del_member` command. The argument is **username**. For example: `/add_member cyandev`.

Currently, only admin users can use admin commands, other member users are not allowed to use them.

### Database

The bot will use SQLite database to store some data produced during runtime. By default, if you don't provide a local file path, the data will be stored in memory database. When you restart the bot, all previous data (such as added members) will be lost. We recommend you to use the file-based database for usability.

## Roadmap

TeleGPT will be actively maintained recently, there are some planned features that are in development.

- [ ] Retry with exponential backoff.
- [ ] Conversation presets.
- [ ] More user-friendly interface for admin operations.
- [ ] Remote controlling with HTTP APIs.

## Contribution

Issues and PRs are welcomed. Before submitting new issues or PRs, it's better to check the existing ones first. Discussions and feature requests are nice to have before you start working on something.

## License

MIT
