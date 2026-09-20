# 7 Days to Die Terraform

State bucket は、この構成自身の state に依存させないため bootstrap で作成する。

```sh
gcloud storage buckets create gs://YOUR_TF_STATE_BUCKET --location=asia-northeast1 --uniform-bucket-level-access
gcloud storage buckets update gs://YOUR_TF_STATE_BUCKET --versioning
cp terraform.tfvars.example terraform.tfvars
# terraform.tfvars のプロジェクト、サービスアカウント、課金アカウント、接続元 /32 を実環境に合わせる。
terraform init -lockfile=readonly -backend-config="bucket=YOUR_TF_STATE_BUCKET" -backend-config="prefix=seven-days-server"
terraform fmt -check
terraform validate
terraform plan -var-file=terraform.tfvars -out=/tmp/seven-days-server.tfplan
terraform apply /tmp/seven-days-server.tfplan
```

`.terraform.lock.hcl` はコミットし、検証済みの provider バージョンとチェックサムを固定する。保存済み plan を apply することで、確認した計画と実際の変更内容を一致させる。

`backup_bucket_name` を空のままにすると、`<project_id>-<instance_name>-backups` という名前でバックアップ用バケットを作成する。バケットは東京リージョンの Standard Storage、30日後削除のライフサイクル、公開アクセス禁止で構成される。`backup_retention_days` で保持日数を変更できる。

Terraformは`server_password_secret_id`の専用Secretを作成するが、パスワード値をstateへ保存しない。VMはゲームサーバーパスワード生成後にこのSecretへversionを追加し、Cloud Runは`/7dtd ip add`時に読み取る。既存VMではapply後に一度起動して同期し、output `server_password_secret_id`と有効なversionを確認してからIP追加を試す。Secretには`prevent_destroy`を設定しているため、削除が必要な場合は値の退避と人手確認を先に行う。

既存の Persistent Disk は容量を縮小できないため、既存の 80 GiB ディスクへこの設定をそのまま apply しない。既存環境では、移行作業が完了するまで `data_disk_size_gb=80` を明示し、別の 10 GiB ディスクへゲームデータをコピーして短時間の動作確認をした後、Terraform のディスク参照を計画的に切り替える。継続運用へ移る場合は20GiB以上へ拡張する。

state bucket の IAM は Terraform 実行者だけに限定する。VM のデフォルトサービスアカウントには、バックアップ用バケットへのオブジェクト作成権限だけを付与する。`terraform destroy` は data disk の `prevent_destroy` により失敗する設計であり、データディスクを明示的に state から外すなどの人手確認なしに解除しない。
