resource "google_compute_disk" "data" {
  name = "${var.instance_name}-data"
  type = "pd-balanced"
  size = 80
  zone = var.zone
  lifecycle {
    prevent_destroy = true
  }
}

resource "google_compute_resource_policy" "snapshots" {
  name = "${var.instance_name}-daily-snapshots"
  region = var.region
  snapshot_schedule_policy {
    schedule {
      daily_schedule {
        days_in_cycle = 1
        start_time = "19:00"
      }
    }
    retention_policy {
      max_retention_days = 14
      on_source_disk_delete = "KEEP_AUTO_SNAPSHOTS"
    }
    snapshot_properties {
      storage_locations = [var.region]
    }
  }
}

resource "google_compute_disk_resource_policy_attachment" "snapshots" {
  name = google_compute_resource_policy.snapshots.name
  disk = google_compute_disk.data.name
  zone = var.zone
}

resource "google_compute_firewall" "game" {
  name = "${var.instance_name}-game"
  network = var.network
  source_ranges = var.game_source_ranges
  target_tags = [var.instance_name]
  allow {
    protocol = "tcp"
    ports = ["26900"]
  }
  allow {
    protocol = "udp"
    ports = ["26900-26903"]
  }
}

resource "google_compute_instance" "server" {
  name = var.instance_name
  zone = var.zone
  machine_type = var.machine_type
  tags = [var.instance_name]
  allow_stopping_for_update = true
  boot_disk {
    initialize_params {
      image = "ubuntu-os-cloud/ubuntu-2404-lts-amd64"
      size = 30
      type = "pd-balanced"
    }
  }
  attached_disk {
    source = google_compute_disk.data.id
    device_name = "seven-days-data"
    mode = "READ_WRITE"
  }
  network_interface {
    network = var.network
    access_config {}
  }
  metadata = {
    enable-osconfig = "TRUE"
    seven-days-safe-stop = base64encode(file("${path.module}/../../seven-days/safe-stop.sh"))
    seven-days-backup = base64encode(file("${path.module}/../../seven-days/backup.sh"))
    seven-days-config = base64encode(file("${path.module}/../../seven-days/serverconfig.template.xml"))
  }
  metadata_startup_script = file("${path.module}/../../seven-days/install.sh")
  service_account {
    scopes = ["cloud-platform"]
  }
  depends_on = [google_compute_disk_resource_policy_attachment.snapshots]
}

resource "google_project_iam_custom_role" "operator" {
  role_id = "sevenDaysInstanceOperator"
  title = "7DTD instance operator"
  permissions = ["compute.instances.get", "compute.instances.start", "compute.instances.stop", "compute.zoneOperations.get"]
}

resource "google_project_iam_member" "cloud_run_operator" {
  project = var.project_id
  role = google_project_iam_custom_role.operator.name
  member = "serviceAccount:${var.cloud_run_service_account_email}"
}

data "google_secret_manager_secret" "duckdns" {
  secret_id = var.duckdns_secret_id
}
resource "google_secret_manager_secret_iam_member" "duckdns" {
  secret_id = data.google_secret_manager_secret.duckdns.id
  role = "roles/secretmanager.secretAccessor"
  member = "serviceAccount:${var.cloud_run_service_account_email}"
}

resource "google_billing_budget" "monthly" {
  billing_account = var.billing_account_id
  display_name = "7DTD monthly budget"
  budget_filter {
    projects = ["projects/${data.google_project.current.number}"]
  }
  amount {
    specified_amount {
      currency_code = "JPY"
      units = tostring(var.monthly_budget_jpy)
    }
  }
  threshold_rules { threshold_percent = 0.5 }
  threshold_rules { threshold_percent = 0.9 }
  threshold_rules { threshold_percent = 1.0 }
}
data "google_project" "current" {}
