# 7DTD status Guest Attributes障害レポート（2026-09-20）

## 概要

7DTD VMを停止後に再起動し、Discordの`/7dtd status`を実行すると、利用者へ「7DTD サーバーの操作に失敗した」と表示された。調査の結果、VMの起動自体は成功していたが、Cloud Runへデプロイ済みのコードが参照するCompute Engine Guest Attributesに対して、IAMと既存VMの設定が未反映だった。

本番のIAM、VMメタデータ、既存VM内のREADY通知処理を同期し、現在の起動に対応する`READY`、ゲームポート、DuckDNS、Terraform driftがないことを確認して復旧した。

## 影響

- VMの起動要求とゲームサーバーの起動は成功していた。
- `/7dtd status`はGuest Attributes取得時に失敗し、VM状態や接続先を表示できなかった。
- READY判定後に実行するDuckDNS更新へ到達しないため、DuckDNSが停止前の外部IPv4を指していた。
- 外部IPv4へ直接接続すればTCP 26900へ到達できる状態だった。

## 確認した事実

1. Cloud Runログ
   - 2026-09-20 02:16 JSTおよび07:28 JSTに`SevenDaysStatus`が`Compute Engine guest attribute lookup failed`で失敗していた。
   - 同時刻帯のCompute Engine監査ログではstart APIが成功しており、起動APIの失敗ではなかった。
2. Compute Engine
   - VMは`RUNNING`で、TCP 26900へ接続できた。
   - `seven-days/runtime-state`のGuest Attributeは存在しなかった。
   - 実環境のメタデータには`enable-guest-attributes`がなく、startup scriptもREADY通知導入前の内容だった。
3. IAM
   - Cloud Run用custom roleには`compute.instances.get`、start、stop、zone operation参照だけがあり、`compute.instances.getGuestAttributes`がなかった。
   - Cloud Runのサービスアカウントには対象custom roleが付与されていた。
4. DNS
   - VMの外部IPv4と`zunda-7dtd.duckdns.org`のAレコードが一致していなかった。
5. Terraform
   - 初回の対象限定planはIAMのインプレース更新に加え、startup script差分によるVMのdestroy/createを提示した。
   - VM置換は本障害の修正に不要で、OSディスクや稼働環境への影響が大きいため適用しなかった。

## 根本原因

アプリケーションコードを先にCloud Runへデプロイし、依存するTerraform/IAMと既存VMの移行を完了していなかったことが原因である。

新規VMでは現在のstartup scriptがGuest Attributes通知用ファイルとunitを配置する。一方、既存VMは`/var/lib/seven-days-provisioned`があるとstartup scriptを冒頭で終了するため、メタデータ上のstartup scriptだけを更新してもREADY通知処理が後付けされない。この既存VM固有の移行条件も未考慮だった。

## 解決方法

### IAMとVMメタデータ

- 保存済みTerraform planを使い、custom roleへ`compute.instances.getGuestAttributes`だけをインプレース追加した。
- VMメタデータへ`enable-guest-attributes=TRUE`を追加した。
- VM置換を含むplanは適用しなかった。

### 既存VMの移行

- `migrate-runtime-state.sh`を一時startup scriptとして設定した。
- 通常停止・起動により、次のファイルを既存VMへ配置した。
  - `/usr/local/sbin/seven-days-state`
  - `/usr/local/sbin/seven-days-ready`
  - READY通知を含む`/etc/systemd/system/seven-days.service`
  - Terraformメタデータと同期した`/usr/local/sbin/seven-days-safe-stop`
- 稼働中unitの強制reloadはせず、次の通常停止・起動で更新済みunitを読み込ませた。
- 移行完了後、VMメタデータのstartup scriptを通常の`infra/seven-days/install.sh`へ戻した。

### DuckDNS

- Secret値を標準出力やURLへ表示せず、Secret ManagerからDuckDNS APIへ直接渡して現在の外部IPv4へ更新した。

## 実施手順と安全上の判断

1. 対象VMのproject、zone、instance名、状態、接続ディスクをread-onlyで確認した。
2. Terraform planを対象リソースに限定し、add/change/destroy件数と差分を確認した。
3. IAMだけの`0 add / 1 change / 0 destroy` planを保存して適用した。
4. Guest Attributesメタデータと一時startup scriptを設定した。
5. VMが停止済みであることを確認して通常起動し、serial logで移行スクリプトのexit status 0を確認した。
6. startup scriptを通常版へ戻し、通常停止・起動で更新済みunitを読み込ませた。
7. Guest Attributeが現在のboot IDと起動時刻を持つ`READY`になるまで確認した。
8. 安全停止スクリプトのメタデータ差分だけになったTerraform planを保存・適用し、同じ移行スクリプトでVM内へ同期した。
9. 最終起動で`READY`、TCP 26900、外部IPv4を確認し、DuckDNSを同期した。
10. 対象限定Terraform planが`No changes`になることを確認した。

作業中にVM削除、ディスク削除、強制停止、TerraformによるVM置換は行っていない。停止はすべてCompute Engineの通常停止を使用し、ゲーム内保存と終了待ちを経由した。

## 復旧確認結果

- VM: `RUNNING`
- データディスク: `seven-days-data`を継続使用
- Guest Attributes: `READY`、現在のboot IDと起動時刻を保持
- TCP 26900: 接続成功
- 外部IPv4: `34.146.183.201`
- DuckDNS Aレコード: `34.146.183.201`で一致
- Cloud Run用custom role: `compute.instances.getGuestAttributes`を保持
- Cloud Runサービスアカウント: 対象custom roleへのbindingを確認
- Terraform対象限定plan: `No changes`

## 採用しなかった対応

- VMをdestroy/createするTerraform apply: 既存環境への影響が大きく、本修正には不要なため不採用。
- アプリ側で権限エラーを`NOT READY`として握りつぶす対応: 設定不備を隠し、READYとDuckDNSが恒久的に更新されないため不採用。
- サービスアカウントへToken Creator権限を追加する対応: 調査のためだけに権限を広げない。custom role定義とIAM bindingの確認で代替した。
- 強制停止や稼働中unitの強制reload: セーブデータ保護と変更範囲の最小化のため不採用。

## 残る確認

Discordが生成する正規署名付きinteractionは外部から再現できないため、Discord上で`/7dtd status`を1回実行し、`VM: RUNNING`、`ゲーム: READY`、接続先が表示されることを最終のエンドツーエンド確認とする。
