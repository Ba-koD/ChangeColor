# ChangeColor Privacy Policy

_Last updated: 2026-11-24_

This policy explains what data the ChangeColor Discord bot ("the bot") collects, why it is collected, and how it is handled. [한국어는 아래에 있습니다.](#개인정보-처리방침-한국어)

## Data we store

The bot stores only what it needs to manage color roles.

**Per server (guild)**
- Server ID
- Role IDs set by server administrators: allowed roles, the position anchor role, and the start/end marker roles
- The removal policy (keep / remove immediately / remove after a grace period)
- A mapping from color values (e.g. `#69ebff`) to the IDs of the color roles the bot created

**Per member who has used `/컬러`**
- Discord user ID
- The last color they chose
- The ID of the color role currently assigned to them
- The time they lost eligibility, if they lost it (used for grace periods and restoring colors)

We do **not** store usernames, nicknames, avatars, message content, presence or activity, voice data, or anything about members who have never used the bot.

## Data we read but do not store

To decide whether a member may use colors, the bot reads that member's current role list from the Discord API. The role list is used only for that check and is not saved.

## How data is stored

- All data is kept in a single file on a server run by the bot operator. The file is encrypted at rest with AES-256-GCM, and the key is kept separately from the file.
- It is not sold, shared with third parties, or used for advertising, analytics, or training.
- Application logs may include server IDs and user IDs while an error is being handled. Logs are used only for debugging.

## Retention and deletion

- Server settings and member records are kept while the bot is in the server, so a member's color can be restored if they become eligible again.
- Members can remove their color at any time with `/컬러 작업:제거`.
- To have all of your data deleted, or a whole server's data deleted, contact us using the details below. We will delete it within 30 days.

## Contact

- GitHub Issues: https://github.com/Ba-koD/ChangeColor/issues

## Changes

When this policy changes, the new version will be published at this address and the "Last updated" date will change.

---

## 개인정보 처리방침 (한국어)

_최종 수정일: 2026-11-24_

### 저장하는 정보

봇은 컬러 역할을 관리하는 데 필요한 정보만 저장합니다.

**서버별 정보**
- 서버 ID
- 관리자가 설정한 역할 ID(허용 역할, 위치 기준 역할, 시작/끝 구분 역할)
- 허용 역할을 잃었을 때 처리 정책(유지 / 즉시 제거 / 유예 후 제거)
- 색상 값과 봇이 만든 컬러 역할 ID의 대응표

**`/컬러`를 사용한 멤버별 정보**
- 디스코드 사용자 ID
- 마지막으로 고른 색상
- 현재 부여된 컬러 역할 ID
- 사용 자격을 잃은 시각(유예 기간 계산과 색상 복구에만 사용)

이름, 닉네임, 프로필 사진, 메시지 내용, 활동 상태, 음성 정보는 저장하지 않습니다. 봇을 한 번도 쓰지 않은 멤버의 정보도 저장하지 않습니다.

### 저장하지 않고 확인만 하는 정보

멤버가 컬러를 쓸 수 있는지 판단할 때 디스코드 API에서 그 멤버의 현재 역할 목록을 조회합니다. 이 목록은 판단에만 쓰고 저장하지 않습니다.

### 보관 방법

- 모든 정보는 운영자가 관리하는 서버의 파일 하나에 저장됩니다. 이 파일은 AES-256-GCM으로 암호화해 저장하며, 키는 파일과 따로 보관합니다.
- 판매하거나 제3자에게 제공하지 않으며, 광고·분석·학습에 쓰지 않습니다.
- 오류를 처리하는 동안 실행 로그에 서버 ID와 사용자 ID가 남을 수 있습니다. 이 로그는 디버깅에만 씁니다.

### 보관 기간과 삭제

- 서버 설정과 멤버 기록은 봇이 서버에 있는 동안 보관합니다. 다시 자격을 얻었을 때 색상을 복구하기 위해서입니다.
- `/컬러 작업:제거`로 언제든 내 컬러를 제거할 수 있습니다.
- 내 정보 전체나 서버 전체 정보의 삭제를 원하면 아래 연락처로 요청해 주세요. 30일 안에 삭제합니다.

### 문의

- GitHub Issues: https://github.com/Ba-koD/ChangeColor/issues
