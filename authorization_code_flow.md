

Чтобы получить access и refresh токены пользователя, с которыми можно будет ходить в DevApi от имени этого самого пользователя, необходимо сперва получить authorization code, который затем обменять отдельным запросом на токены

## Получение authorization code



```bash
https://boosty.to/app/oauth2/authorize
```

Get параметры:

*   **client\_id** \- client id приложения
*   **redirect\_url** \- url\_encoded URL редиректа, должен быть в списке redirect\_urls приложения
*   **response\_type** \- "code" для получения authorization code
*   **state** \- произвольная строка, которая будет передана в неизменном виде при редиректе (нужна для идентификации пользователя на стороне приложения)

Для получения authorization code надо открыть пользователю виджет авторизации с соответствующими приложению параметрами. В после согласия (или несогласия) пользователя на авторизацию, пользователь будет перенаправлен на указанный в параметрах виджета URL редиректа с соответствующими get параметрами:

*   **state, code** - в случае согласия пользователя на авторизацию
*   **state, error, error\_reason** - в случае отказа пользователя от авторизации

Пример:


```bash
https://boosty.to/app/oauth2/authorize?client_id=boosty_project_d4NgSV6n2kI&redirect_url=https://my.website.com/auth&response_type=code&state=some_string

# Пользователь согласился, его перенаправляет на
https://my.website.com/auth?code=1Et0RknwZ9gOgpBm37dHog&state=some_string

# Пользователь отказался, его перенаправляет на
https://my.website.com/auth?state=some_string&error=access_denied&error_reason=user_denied
```



## Обмен authorization code на токены пользователя



```bash
POST https://api.boosty.to/oauth/server/token
```

HTTP заголовки:

*   **Authorization** \- (обязательный) Basic авторизация. Для токена необходимы client\_id и secret приложения. Токен создается по схеме encode\_base64(client\_id + ":" + secret). То есть для client\_id "boosty\_project\_5el6zeBwO6s" и secret "YT3kforZ0V4FD6UHir4umQmeHLLWEU85PQWBRTHIVbHU" токен будет "Basic Ym9vc3R5X3Byb2plY3RfNWVsNnplQndPNnM6WVQza2ZvclowVjRGRDZVSGlyNHVtUW1lSExMV0VVODVQUVdCUlRISVZiSFU="


Get параметры:

*   **grant\_type** \- "authorization\_code" для обмена авторизационного кода на токены пользователя
*   **redirect\_uri** \- URL на который был произошел редирект и передача авторизационного кода
*   **code** \- авторизационный код

Формат ответа


```bash
{
  "access_token": string,
  "expires_in": integer,
  "token_type": string,
  "refresh_token": string
}
```

Пример:


```bash
curl -X 'POST' \
  'https://api.boosty.to/oauth/server/token' \
  -H 'accept: application/json' \
  -H 'Authorization: Basic Ym9vc3R5X3Byb2plY3RfNWVsNnplQndPNnM6WVQza2ZvclowVjRGRDZVSGlyNHVtUW1lSExMV0VVODVQUVdCUlRISVZiSFU=' \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  -d 'grant_type=authorization_code&code=1Et0RknwZ9gOgpBm37dHog&redirect_uri=https%3A%2F%2Fmy.website.com%2Fauth'

{
  "access_token": "PBm8qW23uDpjYyXNhwh0T5BF0mjKKxUZ2GXLV0OQ65E",
  "expires_in": 172800,
  "token_type": "Bearer",
  "refresh_token": "SnmhSh17Rz7WjKD1K1urGSHHYgLRpk1dpAQS6mIA3j0"
}
```



## Обновление access\_token



```bash
POST https://api.boosty.to/oauth/server/token
```

HTTP заголовки:

*   **Authorization** \- (обязательный) Basic авторизация. Для токена необходимы client\_id и secret приложения. Токен создается по схеме encode\_base64(client\_id + ":" + secret). То есть для client\_id "boosty\_project\_5el6zeBwO6s" и secret "YT3kforZ0V4FD6UHir4umQmeHLLWEU85PQWBRTHIVbHU" токен будет "Basic Ym9vc3R5X3Byb2plY3RfNWVsNnplQndPNnM6WVQza2ZvclowVjRGRDZVSGlyNHVtUW1lSExMV0VVODVQUVdCUlRISVZiSFU="


Get параметры:

*   **grant\_type** \- "refresh\_token" для обновления access\_token пользователя
*   **refresh\_token** \-  refresh\_token пользователя

Формат ответа


```bash
{
  "access_token": string,
  "expires_in": integer,
  "token_type": string,
  "refresh_token": string
}
```

Пример:


```bash
curl -X 'POST' \
  'https://api.boosty.to/oauth/server/token' \
  -H 'accept: application/json' \
  -H 'Authorization: Basic Ym9vc3R5X2RzaHZfbVhGUzJhMFdMNjQ6dXVqMDFpYUdRSjdybEZpU3k1ejBVcDJDZ2xmSVhZU1ZEYjZGbW9kcTZmMA==' \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  -d 'grant_type=refresh_token&refresh_token=SnmhSh17Rz7WjKD1K1urGSHHYgLRpk1dpAQS6mIA3j0'

{
  "refresh_token": "2Ngq2TqdDGo5GMZbjeTV5pUO2vSXTTaXDEDplEquvvA",
  "expires_in": 172800,
  "access_token": "oEAWgofeT7pQMprgz25oz2Pzw17xY5uM7DUEUihCybw",
  "token_type": "Bearer"
}
```
