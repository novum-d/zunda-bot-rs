# 7 Days to Die Terraform

State bucket は、この構成自身の state に依存させないため bootstrap で作成する。

```sh
gcloud storage buckets create gs://YOUR_TF_STATE_BUCKET --location=asia-northeast1 --uniform-bucket-level-access
gcloud storage buckets update gs://YOUR_TF_STATE_BUCKET --versioning
terraform init -backend-config="bucket=YOUR_TF_STATE_BUCKET" -backend-config="prefix=seven-days-server"
terraform plan -var-file=terraform.tfvars
terraform apply -var-file=terraform.tfvars
```

`backup_bucket_name` を空のままにすると、`<project_id>-<instance_name>-backups` という名前でバックアップ用バケットを作成する。バケットは東京リージョンの Standard Storage、30日後削除のライフサイクル、公開アクセス禁止で構成される。`backup_retention_days` で保持日数を変更できる。

既存の Persistent Disk は容量を縮小できないため、既存の 80 GiB ディスクへこの設定をそのまま apply しない。既存環境では、移行作業が完了するまで `data_disk_size_gb=80` を明示し、別の 10 GiB ディスクへゲームデータをコピーして短時間の動作確認をした後、Terraform のディスク参照を計画的に切り替える。継続運用へ移る場合は20GiB以上へ拡張する。

state bucket の IAM は Terraform 実行者だけに限定する。VM のデフォルトサービスアカウントには、バックアップ用バケットへのオブジェクト作成権限だけを付与する。`terraform destroy` は data disk の `prevent_destroy` により失敗する設計であり、データディスクを明示的に state から外すなどの人手確認なしに解除しない。
