# VM の名前。Cloud Run や手動運用時の対象指定に利用する。
output "instance_name" { value = google_compute_instance.server.name }

# VM とは独立して保持するゲームデータ用ディスクの名前。
output "data_disk" { value = google_compute_disk.data.name }

# バックアップ先の GCS バケット名。
output "backup_bucket" { value = google_storage_bucket.backup.name }

# Cloud Run の VM 操作用サービスアカウントへ付与したカスタムロール名。
output "cloud_run_custom_role" { value = google_project_iam_custom_role.operator.name }

# Cloud Billing ConsoleでStandard usage cost exportの保存先に指定するdataset。
output "billing_export_dataset" { value = google_bigquery_dataset.billing_export.dataset_id }
