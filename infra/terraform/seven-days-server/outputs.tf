output "instance_name" { value = google_compute_instance.server.name }
output "data_disk" { value = google_compute_disk.data.name }
output "cloud_run_custom_role" { value = google_project_iam_custom_role.operator.name }
