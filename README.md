# Change Color Discord Bot

Discord 서버에서 허용된 역할을 가진 유저가 `/컬러`로 자기 닉네임 색상 역할을 만들고 적용하는 Rust 봇입니다.

## 사용 방식

이미 서비스 중인 봇을 초대해서 쓰거나, 직접 봇 토큰을 발급해서 실행할 수 있습니다.

서비스 중인 봇 초대:

```text
https://discord.com/oauth2/authorize?client_id=703977714868158475&permissions=268435456&scope=bot%20applications.commands
```

직접 실행할 때는 아래 준비/설정/실행 단계를 따라가면 됩니다.

## 준비

직접 실행할 때 필요한 준비입니다.

1. Discord Developer Portal에서 봇을 만들고 토큰을 발급합니다.
2. Bot 설정에서 **Server Members Intent**를 켭니다.
3. 봇 초대 URL에는 `bot`, `applications.commands` scope를 넣습니다.
4. 봇 권한은 최소 `Manage Roles`가 필요합니다.
5. 서버 역할 목록에서 봇 역할을 컬러 역할 묶음이 들어갈 위치보다 위로 올립니다.

## Discord 권한 설정

Developer Portal에서 아래 설정이 필요합니다.

- **Bot > Privileged Gateway Intents > Server Members Intent**: 켜야 합니다. 허용 역할을 잃은 유저의 색상 역할 제거/복구 정책을 처리할 때 필요합니다.
- **OAuth2 > URL Generator > Scopes**: `bot`, `applications.commands`를 선택합니다.
- **OAuth2 > URL Generator > Bot Permissions**: `Manage Roles`를 선택합니다.

직접 실행용 봇의 초대 URL을 만들면 권한 값은 `268435456`입니다.

```text
https://discord.com/oauth2/authorize?client_id=봇_CLIENT_ID&permissions=268435456&scope=bot%20applications.commands
```

서버에 초대한 뒤에는 역할 순서가 중요합니다.

- 봇 역할은 `/컬러설정 위치기준`으로 지정할 역할보다 위에 있어야 합니다.
- 봇 역할은 `======= COLOR START =======`, `======= COLOR END =======`, `#rrggbb` 색상 역할보다 위에 있어야 합니다.
- 봇은 자기 최고 역할보다 높거나 같은 역할을 만들거나 옮기거나 삭제할 수 없습니다.
- 색상 역할은 권한 없이 생성되며, 색상 표시용으로만 사용됩니다.

명령어 권한은 이렇게 동작합니다.

- `/컬러설정`은 Discord 기본 권한도 Administrator로 등록되고, 봇 내부에서도 Administrator 권한을 다시 확인합니다.
- `/컬러`는 서버 관리자가 `/컬러설정 허용역할추가`로 지정한 역할을 가진 유저만 사용할 수 있습니다.
- 사용자가 채널에서 slash command를 쓰려면 Discord의 **Use Application Commands** 권한이 막혀 있으면 안 됩니다.

## 설정

```bash
cp .env.example .env
```

`.env`에 값을 입력합니다.

```env
DISCORD_TOKEN=...
DATA_PATH=./data/state.json
```

봇이 서버에 들어가면 해당 서버의 guild slash command가 자동 등록됩니다. 이미 들어가 있는 서버도 봇 시작 시 다시 등록됩니다. `GUILD_ID`나 `APPLICATION_ID`는 필요 없습니다.

## 실행

로컬:

```bash
cargo run --release
```

Docker:

```bash
docker compose up -d --build
docker compose logs -f
```

상태 JSON은 Docker에서 `./data/state.json`에 저장됩니다.

색상 역할 이름은 `#rrggbb` 형식으로 생성됩니다. 사용자가 색을 바꾸거나 제거했을 때 이전 색상 역할을 쓰는 멤버가 더 없으면 봇이 해당 색상 역할을 삭제합니다.
그라데이션 색상은 `#rrggbb-#rrggbb` 형식의 역할로 생성됩니다. Discord 서버가 `ENHANCED_ROLE_COLORS` 기능을 지원하지 않으면 Discord API 에러 코드와 응답 본문을 포함해 실패 메시지를 보여줍니다.

## 명령어 삭제

이 앱은 실행할 때 현재 코드의 명령어 목록으로 Discord 명령어를 bulk overwrite 합니다. 그래서 같은 앱과 같은 스코프 안에서 코드에서 사라진 명령어는 다음 실행 때 같이 삭제됩니다.

예전에 쓰던 봇/토큰의 명령어가 남아 있으면, 그 명령어는 예전 Discord application 소유라서 새 봇 토큰으로 지울 수 없습니다. 예전 봇의 토큰으로 아래 중 하나를 실행하세요.

```bash
# global 명령어 삭제
cargo run --release -- clear-commands

# global 명령어 삭제
DISCORD_TOKEN=old_bot_token cargo run --release -- clear-global-commands

# 특정 서버 guild 명령어 삭제
DISCORD_TOKEN=old_bot_token cargo run --release -- clear-guild-commands your_guild_id
```

global 명령어는 Discord 클라이언트 캐시 때문에 삭제 후에도 잠깐 보일 수 있습니다. 서버 전용 guild 명령어는 보통 바로 반영됩니다.

## 명령어

- `/컬러설정 허용역할추가 role:<역할>`
- `/컬러설정 허용역할제거 role:<역할>`
- `/컬러설정 정책 mode:<유지|즉시제거|유예제거> grace_days:<0..7>`
- `/컬러설정 위치기준 role:<역할>`
- `/컬러설정 재정렬`
- `/컬러설정 상태`
- `/컬러 hex:<#rrggbb>`
- `/컬러 시작:<#rrggbb> 끝:<#rrggbb>`
- `/컬러 작업:제거`
- `/컬러 작업:복구`

`/컬러설정`은 Administrator 권한이 있는 서버 관리자만 사용할 수 있습니다.

## 역할 위치 주의

Discord는 유저가 가진 역할 중 가장 높은 색상 역할의 색을 표시합니다. `/컬러설정 위치기준`으로 기준 역할을 지정하면 봇이 `======= COLOR START =======`와 `======= COLOR END =======` 사이에 컬러 역할을 모아 기준 역할 바로 위로 옮깁니다.

봇은 자기 최고 역할보다 낮은 역할만 관리할 수 있습니다. 기준 역할이나 컬러 역할 묶음이 봇 역할보다 높으면 재정렬과 역할 부여가 실패합니다.

기본 설정 순서는 아래처럼 잡으면 됩니다.

1. 봇을 `bot`, `applications.commands`, `Manage Roles`로 초대합니다.
2. 서버 역할 설정에서 봇 역할을 컬러 역할 묶음이 들어갈 위치보다 위로 올립니다.
3. `/컬러설정 위치기준 role:<기준역할>`을 실행합니다.
4. `/컬러설정 허용역할추가 role:<컬러 사용 가능 역할>`을 실행합니다.
5. 필요하면 `/컬러설정 정책`으로 허용 역할을 잃었을 때 색상 역할 처리 방식을 정합니다.
6. `/컬러설정 상태`로 설정과 역할 위치를 확인합니다.
