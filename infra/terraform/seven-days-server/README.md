# 7 Days to Die Terraform

State bucket は、この構成自身の state に依存させないため bootstrap で作成する。

```sh
gcloud storage buckets create gs://YOUR_TF_STATE_BUCKET --location=asia-northeast1 --uniform-bucket-level-access
gcloud storage buckets update gs://YOUR_TF_STATE_BUCKET --versioning
cp terraform.tfvars.example terraform.tfvars
# terraform.tfvars のプロジェクト、サービスアカウント、課金アカウント、ゲームポートの接続元を実環境に合わせる。
terraform init -lockfile=readonly -backend-config="bucket=YOUR_TF_STATE_BUCKET" -backend-config="prefix=seven-days-server"
terraform fmt -check
terraform validate
terraform plan -var-file=terraform.tfvars -out=/tmp/seven-days-server.tfplan
terraform apply /tmp/seven-days-server.tfplan
```

`.terraform.lock.hcl` はコミットし、検証済みの provider バージョンとチェックサムを固定する。保存済み plan を apply することで、確認した計画と実際の変更内容を一致させる。

`game_source_ranges = ["0.0.0.0/0"]` はゲーム用 TCP 26900 と UDP 26900–26903 を全IPv4から接続可能にする。SSHなど他のポートはこのルールで公開しない。接続元を限定する場合はIPv4アドレスを `/32` で指定する。

`backup_bucket_name` を空のままにすると、`<project_id>-<instance_name>-backups` という名前でバックアップ用バケットを作成する。バケットは東京リージョンの Standard Storage、30日後削除のライフサイクル、公開アクセス禁止で構成される。`backup_retention_days` で保持日数を変更できる。

`duckdns_subdomain`を空のままにすると`instance_name`をDuckDNSのサブドメインとして使用する。異なる名前を使用する場合だけ明示する。VMはゲームのREADY時にSecret Managerからtokenをメモリ上へ取得し、現在の一時外部IPv4をDuckDNSへ同期する。token自体はTerraform stateやVM metadataへ保存しない。

`metadata_startup_script`は新規VMの初期構築専用で、内容変更による既存VMの置換を避けるためTerraformの差分検出対象外にしている。既存VMへruntime helperを反映するときはRunbookの移行手順を使い、一時startup scriptを通常版へ必ず戻す。

apply後、Cloud Billing ConsoleのBigQuery exportでStandard usage costを有効化し、output `billing_export_dataset` のdatasetを保存先に指定する。TerraformはdatasetとCloud Runの読み取りIAMを作成するが、Billing Account側のexport有効化は行わない。作成されたtable名をCloud Runの `SEVEN_DAYS_BILLING_TABLE` へ設定する。初回反映と通常の利用料金反映には遅延がある。

既存の Persistent Disk は容量を縮小できないため、既存の 80 GiB ディスクへこの設定をそのまま apply しない。既存環境では、移行作業が完了するまで `data_disk_size_gb=80` を明示し、別の 10 GiB ディスクへゲームデータをコピーして短時間の動作確認をした後、Terraform のディスク参照を計画的に切り替える。継続運用へ移る場合は20GiB以上へ拡張する。

state bucket の IAM は Terraform 実行者だけに限定する。VM のデフォルトサービスアカウントには、バックアップ用バケットへのオブジェクト作成権限と対象DuckDNS SecretだけのAccessorを付与する。`terraform destroy` は data disk の `prevent_destroy` により失敗する設計であり、データディスクを明示的に state から外すなどの人手確認なしに解除しない。
