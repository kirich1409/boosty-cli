# boosty-cli

Локальный CLI для Boosty DevAPI. Он читает данные блога приложения ключом приложения и данные пользователя токеном из authorization-code flow. Это не веб-API и не обещание постов, комментариев или cookie-сессии.

## Команды

- `boosty auth` — Вход и токен.
- `boosty auth login` — Открыть браузер и сохранить токен. Единственная интерактивная команда. Агент забирает данные ключом или уже существующим файлом токенов и логин для этого не запускает.
- `boosty auth refresh` — Обновить токен.
- `boosty auth status` — Показать срок токена и путь к файлу.
- `boosty user` — Показать текущего пользователя. Только `Authorization: Bearer` из файла токенов.
- `boosty subscription-levels` — Показать уровни подписки.
- `boosty subscriptions` — Показать подписки.
- `boosty subscribers [--is-free true|false] [--limit N] [--offset N]` — Показать подписчиков. `--is-free`: «Только бесплатные при true, только платные при false». `--limit`: «Лимит записей». `--offset`: «Смещение следующей страницы». Один запуск делает один запрос и одну страницу. Страницы внутри команды не обходятся. Смещение из ответа передаётся обратно как `--offset`.

API key, client secret, access token и refresh token не принимаются как флаг или позиционный аргумент.

## Вывод

`--json` — глобальный флаг. Он работает и до, и после подкоманды. С ним агент и скрипт разбирают stdout. Без него человеческий stdout — русский текст, не дамп JSON и не колонка сырых имён полей. Имена полей в `--json` не переименовываются. Порядок ключей не обещается.

```
cargo run -- --json user
cargo run -- user --json
```

## Переменные окружения

Значения здесь не задаются.

- `BOOSTY_API_KEY` — секрет приложения, заголовок `X-Api-Key`, нужен для `subscription-levels`, `subscriptions` и `subscribers`. Ответ относительно блога приложения. Аргумента блога нет.
- `BOOSTY_CLIENT_ID` и `BOOSTY_CLIENT_SECRET` — учётные данные token endpoint. Секрет только для `Authorization: Basic`. `BOOSTY_CLIENT_ID` — тот же id, что query `client_id` на authorize.
- `BOOSTY_REDIRECT_URL` — зарегистрированный redirect URL. Та же строка уходит как `redirect_url` на authorize и как `redirect_uri` на token. Имена параметров не сливаются.
- Токен пользователя лежит в файле, не в окружении. По умолчанию `$XDG_CONFIG_HOME/boosty-cli/tokens.json`, а если `XDG_CONFIG_HOME` не задан — `~/.config/boosty-cli/tokens.json`, в том числе на macOS. Путь переопределяет `BOOSTY_TOKEN_PATH`. На Application Support путь не переключается.
- `BOOSTY_AUTHORIZE_URL`, `BOOSTY_TOKEN_URL` и `BOOSTY_DEVAPI_BASE` — подмена для тестов и локальных заглушек, не обязательная настройка обычного запуска.

Три команды ключа отправляют только `X-Api-Key`. `user` отправляет только `Authorization: Bearer`. Token endpoint принимает только Basic. Заголовки не подменяются друг другом.

## Сборка из исходников

Из корня репозитория:

```
cargo build
cargo run --
```

`cargo build` собирает бинарник `boosty`.

## Лицензия

Код доступен по двойной лицензии `MIT OR Apache-2.0`. Тексты: `LICENSE-MIT` и `LICENSE-APACHE`.
