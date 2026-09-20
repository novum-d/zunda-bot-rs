# VM の名前。Cloud Run や手動運用時の対象指定に利用する。
output "instance_name" { value = google_compute_instance.server.name }

# VM とは独立して保持するゲームデータ用ディスクの名前。
output "data_disk" { value = google_compute_disk.data.name }

# バックアップ先の GCS バケット名。
output "backup_bucket" { value = google_storage_bucket.backup.name }

# Cloud Run の VM 操作用サービスアカウントへ付与したカスタムロール名。
output "cloud_run_custom_role" { value = google_project_iam_custom_role.operator.name }

# 値は出力せず、Cloud Run設定に使うゲームサーバーパスワードのSecret IDだけを出力する。
output "server_password_secret_id" { value = google_secret_manager_secret.server_password.secret_id }
