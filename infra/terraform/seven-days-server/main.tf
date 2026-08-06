# ゲームデータを VM のライフサイクルから分離するための永続ディスク。
# prevent_destroy により、Terraform の destroy や VM の再作成でデータを誤削除しない。
resource "google_compute_disk" "data" {
  name = "${var.instance_name}-data"
  type = "pd-balanced"
  size = 80
  zone = var.zone
  lifecycle {
    prevent_destroy = true
  }
}

# 現在は毎日自動でスナップショットを取得し、14 日間保持する。
# 要件では、ゲームの正常停止と保存完了後にだけ取得する方式へ変更する。
# 定期実行の snapshot policy を廃止する際は、このリソースと下記 attachment を見直す。
resource "google_compute_resource_policy" "snapshots" {
  name   = "${var.instance_name}-daily-snapshots"
  region = var.region
  snapshot_schedule_policy {
    schedule {
      daily_schedule {
        days_in_cycle = 1
        start_time    = "19:00"
      }
    }
    retention_policy {
      max_retention_days    = 14
      on_source_disk_delete = "KEEP_AUTO_SNAPSHOTS"
    }
    snapshot_properties {
      storage_locations = [var.region]
    }
  }
}

# 上記のスナップショットポリシーをデータディスクへ適用する。
resource "google_compute_disk_resource_policy_attachment" "snapshots" {
  name = google_compute_resource_policy.snapshots.name
  disk = google_compute_disk.data.name
  zone = var.zone
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
    enable-osconfig      = "TRUE"
    seven-days-safe-stop = base64encode(file("${path.module}/../../seven-days/safe-stop.sh"))
    seven-days-backup    = base64encode(file("${path.module}/../../seven-days/backup.sh"))
    seven-days-config    = base64encode(file("${path.module}/../../seven-days/serverconfig.template.xml"))
  }
  # VM 初回起動時に SteamCMD、7DTD、systemd などをセットアップする。
  metadata_startup_script = file("${path.module}/../../seven-days/install.sh")

  # 現在は Compute Engine のデフォルトサービスアカウントを使用する。
  # cloud-platform は広いスコープなので、VM 側で GCP API を使う処理が固まったら縮小する。
  service_account {
    scopes = ["cloud-platform"]
  }
  # 現在の構成では snapshot policy の attachment 完了後に VM を作成する。
  # 停止後の明示的なスナップショット方式へ移行する際は削除対象。
  depends_on = [google_compute_disk_resource_policy_attachment.snapshots]
}

# Cloud Run から VM の状態確認・起動・停止だけを行うためのカスタムロール。
# プロジェクトへ付与するため、同じプロジェクト内の他 VM も対象になり得る。
# 専用プロジェクトまたは VM 単位の IAM を検討する。
resource "google_project_iam_custom_role" "operator" {
  role_id     = "sevenDaysInstanceOperator"
  title       = "7DTD instance operator"
  permissions = ["compute.instances.get", "compute.instances.start", "compute.instances.stop", "compute.zoneOperations.get"]
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
