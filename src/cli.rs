use clap::{ArgAction, Parser, Subcommand};

const HELP_TEMPLATE: &str = "{about}\n\nИспользование: {usage}\n\n{all-args}\n";

#[derive(Parser, Debug)]
#[command(
    name = "boosty",
    about = "Клиент Boosty DevAPI",
    disable_help_flag = true,
    disable_help_subcommand = true,
    disable_version_flag = true,
    disable_colored_help = true,
    subcommand_required = true,
    override_usage = "boosty [ПАРАМЕТРЫ] <КОМАНДА>",
    help_template = HELP_TEMPLATE,
    subcommand_help_heading = "Команды"
)]
pub(crate) struct Cli {
    #[arg(
        long,
        global = true,
        action = ArgAction::SetTrue,
        help = "Печатать JSON на stdout",
        help_heading = "Параметры"
    )]
    pub json: bool,

    #[arg(
        short = 'h',
        long = "help",
        action = ArgAction::Help,
        help = "Показать справку",
        help_heading = "Параметры"
    )]
    help: Option<bool>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    #[command(
        about = "Вход и токен",
        disable_help_flag = true,
        disable_help_subcommand = true,
        disable_version_flag = true,
        disable_colored_help = true,
        subcommand_required = true,
        override_usage = "boosty auth [ПАРАМЕТРЫ] <КОМАНДА>",
        help_template = HELP_TEMPLATE,
        subcommand_help_heading = "Команды"
    )]
    Auth {
        #[arg(
            short = 'h',
            long = "help",
            action = ArgAction::Help,
            help = "Показать справку",
            help_heading = "Параметры"
        )]
        help: Option<bool>,

        #[command(subcommand)]
        action: AuthCommand,
    },
}

#[derive(Subcommand, Debug)]
pub(crate) enum AuthCommand {
    #[command(
        about = "Открыть браузер и сохранить токен",
        disable_help_flag = true,
        disable_version_flag = true,
        disable_colored_help = true,
        override_usage = "boosty auth login [ПАРАМЕТРЫ]",
        help_template = HELP_TEMPLATE
    )]
    Login {
        #[arg(
            short = 'h',
            long = "help",
            action = ArgAction::Help,
            help = "Показать справку",
            help_heading = "Параметры"
        )]
        help: Option<bool>,
    },
    #[command(
        about = "Обновить токен",
        disable_help_flag = true,
        disable_version_flag = true,
        disable_colored_help = true,
        override_usage = "boosty auth refresh [ПАРАМЕТРЫ]",
        help_template = HELP_TEMPLATE
    )]
    Refresh {
        #[arg(
            short = 'h',
            long = "help",
            action = ArgAction::Help,
            help = "Показать справку",
            help_heading = "Параметры"
        )]
        help: Option<bool>,
    },
    #[command(
        about = "Показать срок токена и путь к файлу",
        disable_help_flag = true,
        disable_version_flag = true,
        disable_colored_help = true,
        override_usage = "boosty auth status [ПАРАМЕТРЫ]",
        help_template = HELP_TEMPLATE
    )]
    Status {
        #[arg(
            short = 'h',
            long = "help",
            action = ArgAction::Help,
            help = "Показать справку",
            help_heading = "Параметры"
        )]
        help: Option<bool>,
    },
}

pub(crate) fn try_parse() -> Result<Cli, clap::Error> {
    Cli::try_parse()
}
