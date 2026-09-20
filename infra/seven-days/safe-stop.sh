#!/usr/bin/env bash
# systemd の停止処理から呼び出し、ゲーム内の保存と安全なシャットダウンを行う。
#
# systemdから単純にプロセスを強制終了すると、ワールド保存が完了せず、
# データ破損や巻き戻りが起きる可能性があるため、ゲーム内の正式な停止コマンドでサーバーを終了する

set -euo pipefail

# 環境変数で接続先とパスワードファイルを変更できるようにする。
TELNET_HOST=${TELNET_HOST:-127.0.0.1}
TELNET_PORT=${TELNET_PORT:-8081}
TELNET_PASSWORD_FILE=${TELNET_PASSWORD_FILE:-/srv/seven-days-data/telnet-password}

# パスワードファイルがない場合は telnet 操作をスキップし、プロセス終了の待機だけを行う。
if [ -r "$TELNET_PASSWORD_FILE" ]; then
  PASSWORD=$(<"$TELNET_PASSWORD_FILE")

  # パスワード入力後に保存を実行し、30秒の告知時間を置いてからサーバーを停止する。
  if ! {
    printf '%s\n' "$PASSWORD"
    sleep 1
    printf 'say Server shutting down in 30 seconds\nsaveworld\n'
    sleep 30
    printf 'shutdown\n'
  } | timeout 40 telnet "$TELNET_HOST" "$TELNET_PORT" >/dev/null; then
    printf 'Telnet による安全停止要求が完了しませんでした\n' >&2
  fi
fi

# 通知時間を含めて Compute Engine の通常停止猶予120秒以内へ収めるため、
# 停止要求後のプロセス終了待ちは最大64秒とする。
for _ in $(seq 1 32); do
  pgrep -f '7DaysToDieServer' >/dev/null || exit 0
  sleep 2
done

# 期限内に終了しなければ systemd に停止失敗として通知する。
exit 1
