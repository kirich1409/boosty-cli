

## Методы DevAPI

Существует два варианта авторизации при выполнении запросов в DevAPI:

*   **API Key** - в качестве которого используется secret приложения. Для этого его надо передать в заголовке X-Api-Key
*   **Bearer Access Token** - Токен пользователя, авторизовавшегося в приложении через Boosty (см. раздел Authorization Code Flow)

Каждый из методов API принимает один из вариантов авторизации. Это описано в заголовках, которые метод поддерживает. При выполнении запроса с API Key ответ возвращается относительно блога приложения. Для Access Token ответ будет относительно пользователя, для которого токен был выдан

### Информация о юзере приложения



```bash
GET https://devapi.boosty.to/v1/user
```

Авторизационный HTTP заголовок, один из:

*   **Authorization** \- с Bearer токеном пользователя

Формат ответа:


```bash
{
  "user": {
    "id": integer,
    "name": string,
    "email": string,
    "avatarUrl": string,
    "flags": {
      "isAuthor": boolean,
      "isBanned": boolean
    }
  }
}
```

Пример запроса:


```bash
curl -X 'GET' \
  'https://devapi.boosty.to/v1/user' \
  -H "X-Api-Key: fR03QceFPUN8dqm1qSgoDWtJg5FP2EXfdQEHY80vbPQ" # Или авторизация по Bearer токену юзера -H "Authorization: Bearer PBm8qW23uDpjYyXNhwh0T5BF0mjKKxUZ2GXLV0OQ65E"

{
  "user": {
    "id": 42,
    "name": "John Doe",
    "email": "mail@example.com",
    "avatarUrl": "https://images.boosty.to/user/42/avatar?change_time=1765755225",
    "flags": {
      "isAuthor": true,
      "isBanned": false
    }
  }
}
```



### Информация об уровне подписки



```bash
GET https://devapi.boosty.to/v1/subscription_levels
```

Авторизационный HTTP заголовок:

*   **X-Api-Key** \- (обязательный) secret приложения


Формат ответа:


```bash
{
  "subscriptionLevels": [
    {
      "id": integer,
      "price": string,
      "authorId": integer,
      "name": string,
      "createTime": integer,
      "isDeleted": boolean
    },
    ...
    {
      "id": integer,
      "price": string,
      "authorId": integer,
      "name": string,
      "createTime": integer,
      "isDeleted": boolean
    }
  ]
}
```

Пример запроса:


```bash
curl -X 'GET' \
  'https://devapi.boosty.to/v1/subscription_levels' \
  -H "X-Api-Key: fR03QceFPUN8dqm1qSgoDWtJg5FP2EXfdQEHY80vbPQ"

{
  "subscriptionLevels": [
    {
      "id": 63203,
      "price": "RUB0.00",
      "authorId": 42,
      "name": "Default Free Level",
      "createTime": 1550831073,
      "isDeleted": false
    },
    {
      "id": 66207,
      "price": "RUB11.00",
      "authorId": 42,
      "name": "Чирик +1",
      "createTime": 1645548044,
      "isDeleted": false
    },
    {
      "id": 64201,
      "price": "RUB200.00",
      "authorId": 42,
      "name": "Две сотки",
      "createTime": 1579265803,
      "isDeleted": true
    },
    {
      "id": 66699,
      "price": "RUB300.00",
      "authorId": 42,
      "name": "Тракторист",
      "createTime": 1666087074,
      "isDeleted": false
    }
  ]
}
```



### Информация об подписках



```bash
GET https://devapi.boosty.to/v1/subscriptions
```

Авторизационный HTTP заголовок:

*   **X-Api-Key** \- (обязательный) secret приложения


Формат ответа:


```bash
{
  "subscriptions": [
    {
      "id": integer,
      "ownerId": integer,
      "levelId": integer,
      "startTime": integer,
      "endTime": integer,
      "nextPayTime": integer,
      "subscriberId": integer,
      "payments": string
    },
    ...
    {
      "id": integer,
      "ownerId": integer,
      "levelId": integer,
      "startTime": integer,
      "endTime": integer,
      "nextPayTime": integer,
      "subscriberId": integer,
      "payments": string
    }
  ],
  "subscriptionLevels": [
    {
      "id": integer,
      "price": string,
      "authorId": integer,
      "name": string,
      "createTime": integer,
      "isDeleted": boolean
    },
    ...
    {
      "id": integer,
      "price": string,
      "authorId": integer,
      "name": string,
      "createTime": integer,
      "isDeleted": boolean
    }
  ]
}
```

Пример запроса:


```bash
curl -X 'GET' \
  'https://devapi.boosty.to/v1/subscriptions' \
  -H "X-Api-Key: fR03QceFPUN8dqm1qSgoDWtJg5FP2EXfdQEHY80vbPQ"

{
  "subscriptions": [
    {
      "id": 10004177,
      "ownerId": 27901,
      "levelId": 66145,
      "startTime": 1697460684,
      "endTime": 0,
      "nextPayTime": 0,
      "subscriberId": 7,
      "payments": "0.00"
    },
    {
      "id": 762,
      "ownerId": 23601,
      "levelId": 66054,
      "startTime": 1633005914,
      "endTime": 0,
      "nextPayTime": 0,
      "subscriberId": 7,
      "payments": "0.00"
    },
    {
      "id": 10002830,
      "ownerId": 18,
      "levelId": 41,
      "startTime": 1693325607,
      "endTime": 0,
      "nextPayTime": 0,
      "subscriberId": 7,
      "payments": "0.00"
    }
  ],
  "subscriptionLevels": [
    {
      "id": 41,
      "price": "RUB0.00",
      "authorId": 18,
      "name": "Follower",
      "createTime": 1551699878,
      "isDeleted": false
    },
    {
      "id": 66054,
      "price": "RUB0.00",
      "authorId": 23601,
      "name": "Follower",
      "createTime": 1633005664,
      "isDeleted": false
    },
    {
      "id": 66145,
      "price": "RUB0.00",
      "authorId": 27901,
      "name": "Follower",
      "createTime": 1641903542,
      "isDeleted": false
    }
  ]
}
```



### Информация об подписчиках



```bash
GET https://devapi.boosty.to/v1/subscribers
```

Авторизационный HTTP заголовок:

*   **X-Api-Key** \- (обязательный) secret приложения


Get параметры:

*   **is_free** \- (опционально) Если этого параметра нет, то в ответе будут все подписчики. Если параметр со значением "true", то в ответе будут только фолловеры (подписчики на бесплатном уровне). Если значение "false", то только платные подписчики

*   **limit** \- (опционально) целое число, лимит записей в результате

*   **offset** \- (опционально) целое число, новое значение оффсета, для получения следующей пачки подписчиков, возвращается в поле nextOffset результата

Формат ответа:


```bash
{
  "users": [
    {
      "id": integer,
      "name": string,
      "email": string,
      "avatarUrl": string,
      "flags": {
        "isAuthor": boolean,
        "isBanned": boolean
      }
    },
    ...
    {
      "id": integer,
      "name": string,
      "email": string,
      "avatarUrl": string,
      "flags": { 
        "isAuthor": boolean,
        "isBanned": boolean 
      }
    }
  ],
  "subscriptions": [
    {
      "id": integer,
      "ownerId": integer,
      "levelId": integer,
      "startTime": integer,
      "endTime": integer,
      "nextPayTime": integer,
      "subscriberId": integer,
      "payments": string
    },
    ...
    {
      "id": integer,
      "ownerId": integer,
      "levelId": integer,
      "startTime": integer,
      "endTime": integer,
      "nextPayTime": integer,
      "subscriberId": integer,
      "payments": string
    }
  ],
  "extra": {
    "nextOffset": integer
  }
}
```

Пример запроса:


```bash
curl -X 'GET' \
  'https://devapi.boosty.to/v1/subscribers?limit=5&offset=2' \
  -H "X-Api-Key: fR03QceFPUN8dqm1qSgoDWtJg5FP2EXfdQEHY80vbPQ"

{
  "users": [
    {
      "id": 28,
      "name": "Jane Doe",
      "email": "jane_doe@example.com",
      "avatarUrl": "https://images.boosty.to/user/28/avatar?change_time=1681177595",
      "flags": {
        "isAuthor": true,
        "isBanned": false
      }
    },
    {
      "id": 770,
      "name": "Alice",
      "email": "alice@example.com",
      "avatarUrl": "https://images.boosty.to/user/770/avatar?change_time=1594161889",
      "flags": {
        "isAuthor": true,
        "isBanned": false
      }
    },
    {
      "id": 771,
      "name": "Man In The Middle",
      "email": "mitm@example.com",
      "avatarUrl": "https://images.boosty.to/user/771/avatar?change_time=1594167110",
      "flags": {
        "isAuthor": true,
        "isBanned": false
      }
    },
    {
      "id": 772,
      "name": "Bob",
      "email": "bob@example.com",
      "avatarUrl": "https://images.boosty.to/user/772/avatar?change_time=1594167346",
      "flags": {
        "isAuthor": true,
        "isBanned": false
      }
    },
    {
      "id": 774,
      "name": "Charlie",
      "email": "charlie@example.com",
      "avatarUrl": "https://images.boosty.to/user/774/avatar?change_time=1594834598",
      "flags": {
        "isAuthor": true,
        "isBanned": false
      }
    }
  ],
  "subscriptions": [
    {
      "id": 253,
      "ownerId": 7,
      "levelId": 10,
      "startTime": 1556105321,
      "endTime": 0,
      "nextPayTime": 0,
      "subscriberId": 28,
      "payments": "0.00"
    },
    {
      "id": 764,
      "ownerId": 7,
      "levelId": 10,
      "startTime": 1595500599,
      "endTime": 0,
      "nextPayTime": 0,
      "subscriberId": 770,
      "payments": "0.00"
    },
    {
      "id": 786,
      "ownerId": 7,
      "levelId": 10,
      "startTime": 1595500612,
      "endTime": 0,
      "nextPayTime": 0,
      "subscriberId": 771,
      "payments": "0.00"
    },
    {
      "id": 822,
      "ownerId": 7,
      "levelId": 10,
      "startTime": 1595500624,
      "endTime": 0,
      "nextPayTime": 0,
      "subscriberId": 772,
      "payments": "0.00"
    },
    {
      "id": 834,
      "ownerId": 7,
      "levelId": 10,
      "startTime": 1595500634,
      "endTime": 0,
      "nextPayTime": 0,
      "subscriberId": 774,
      "payments": "0.00"
    }
  ],
  "extra": {
    "nextOffset": 834
  }
}
```
