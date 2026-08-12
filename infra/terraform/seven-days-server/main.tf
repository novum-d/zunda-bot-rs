# ゲームデータを VM のライフサイクルから分離するための永続ディスク。
# prevent_destroy により、Terraform の destroy や VM の再作成でデータを誤削除しない。
resource "google_compute_disk" "data" {
  name = "${var.instance_name}-data"
  type = "pd-balanced"
  size = var.data_disk_size_gb
  zone = var.zone
  lifecycle {
    prevent_destroy = true
  }
}

# ゲームのバックアップは VM のディスクとは別の GCS バケットへ保存する。
# ライフサイクルで古いバックアップを削除し、データディスクの容量増加を防ぐ。
resource "google_storage_bucket" "backup" {
  name                        = var.backup_bucket_name != "" ? var.backup_bucket_name : "${var.project_id}-${var.instance_name}-backups"
  location                    = var.region
  storage_class               = "STANDARD"
  uniform_bucket_level_access = true
  public_access_prevention    = "enforced"
  force_destroy               = false

  lifecycle_rule {
    action {
      type = "Delete"
    }
    condition {
      age = var.backup_retention_days
    }
  }
}

# VM は Compute Engine のデフォルトサービスアカウントでバックアップをアップロードする。
# バケット単位の権限に限定し、プロジェクト全体の Storage 権限は付与しない。
resource "google_storage_bucket_iam_member" "backup_writer" {
  bucket = google_storage_bucket.backup.name
  role   = "roles/storage.objectCreator"
  member = "serviceAccount:${data.google_project.current.number}-compute@developer.gserviceaccount.com"
}

# 7DTD のゲーム通信だけを許可するファイアウォール。
# source_ranges は参加者の固定 IP (/32) を指定することを想定する。
# 0.0.0.0/0 を使う場合でも、ここで指定したゲームポート以外は公開しない。
resource "google_compute_firewall" "game" {
  name          = "${var.instance_name}-game"
  network       = var.network
  source_ranges = var.game_source_ranges
  target_tags   = [var.instance_name]
  allow {
    protocol = "tcp"
    ports    = ["26900"]
  }
  allow {
    protocol = "udp"
    ports    = ["26900-26903"]
  }
}

# 7DTD サーバー VM。本体の OS とゲームバイナリは boot disk、
# セーブデータやワールドは上記の分離データディスクに保存する。
resource "google_compute_instance" "server" {
  name         = var.instance_name
  zone         = var.zone
  machine_type = var.machine_type
  tags         = [var.instance_name]
  # マシンタイプなどの変更時に Terraform が VM を停止して更新できる。
  # apply 前にゲームを正常停止し、停止時間を把握しておく。
  allow_stopping_for_update = true
  # Ubuntu とゲームサーバー本体を置く OS ディスク。
  boot_disk {
    initialize_params {
      image = "ubuntu-os-cloud/ubuntu-2404-lts-amd64"
      size  = 30
      type  = "pd-balanced"
    }
  }
  # VM を作り直してもゲームデータを再利用できるよう、分離ディスクを接続する。
  attached_disk {
    source      = google_compute_disk.data.id
    device_name = "seven-days-data"
    mode        = "READ_WRITE"
  }
  # access_config を空で指定すると一時外部 IPv4 が付与される。
  # DuckDNS はこの IP を接続先として案内するために使用する。
  network_interface {
    network = var.network
    access_config {}
  }
  # 起動スクリプトが systemd の停止処理や設定ファイルを配置するための入力。
  # Base64 は暗号化ではないため、秘密情報はここへ入れない。
  # install.sh 側で各値を Base64 デコードして利用する。
  metadata = {
    enable-osconfig          = "TRUE"
    enable-guest-attributes  = "TRUE"
    seven-days-safe-stop     = base64encode(file("${path.module}/../../seven-days/safe-stop.sh"))
    seven-days-backup        = base64encode(file("${path.module}/../../seven-days/backup.sh"))
    seven-days-backup-bucket = base64encode(google_storage_bucket.backup.name)
    seven-days-config        = base64encode(file("${path.module}/../../seven-days/serverconfig.template.xml"))
  }
  # VM 初回起動時に SteamCMD、7DTD、systemd などをセットアップする。
  metadata_startup_script = file("${path.module}/../../seven-days/install.sh")

  # 現在は Compute Engine のデフォルトサービスアカウントを使用する。
  # cloud-platform は広いスコープなので、VM 側で GCP API を使う処理が固まったら縮小する。
  service_account {
    scopes = ["cloud-platform"]
  }
}

# Cloud Run から VM の状態確認・起動・停止だけを行うためのカスタムロール。
# プロジェクトへ付与するため、同じプロジェクト内の他 VM も対象になり得る。
# 専用プロジェクトまたは VM 単位の IAM を検討する。
resource "google_project_iam_custom_role" "operator" {
  role_id = "sevenDaysInstanceOperator"
  title   = "7DTD instance operator"
  permissions = [
    "compute.firewalls.create",
    "compute.firewalls.delete",
    "compute.firewalls.get",
    "compute.firewalls.list",
    "compute.instances.get",
    "compute.instances.getGuestAttributes",
    "compute.instances.start",
    "compute.instances.stop",
    "compute.zoneOperations.get",
  ]
}

# Cloud Run のサービスアカウントへ上記ロールを付与する。
resource "google_project_iam_member" "cloud_run_operator" {
  project = var.project_id
  role    = google_project_iam_custom_role.operator.name
  member  = "serviceAccount:${var.cloud_run_service_account_email}"
}

# Secret の値は Terraform で読み取らず、Secret の存在だけを参照する。
# DuckDNS トークンを Cloud Run が取得できるよう accessor 権限を付与する。
data "google_secret_manager_secret" "duckdns" {
  secret_id = var.duckdns_secret_id
}
# Secret の中身やトークン自体は Terraform の設定・ログへ書かない。
resource "google_secret_manager_secret_iam_member" "duckdns" {
  secret_id = data.google_secret_manager_secret.duckdns.id
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${var.cloud_run_service_account_email}"
}

# 月額予算の通知設定。閾値を超えても自動的に課金や VM を停止する機能ではない。
resource "google_billing_budget" "monthly" {
  billing_account = var.billing_account_id
  display_name    = "7DTD monthly budget"
  budget_filter {
    projects = ["projects/${data.google_project.current.number}"]
  }
  amount {
    specified_amount {
      currency_code = "JPY"
      units         = tostring(var.monthly_budget_jpy)
    }
  }
  threshold_rules { threshold_percent = 0.5 }
  threshold_rules { threshold_percent = 0.9 }
  threshold_rules { threshold_percent = 1.0 }
}

# budget_filter で現在の provider project を対象にするためのプロジェクト情報。
data "google_project" "current" {}
