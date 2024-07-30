#!/bin/bash
set -e

USER=$(whoami)
WG_INTERFACE="wg0"
SUDOERS_FILE="/etc/sudoers"
BACKUP_FILE="$SUDOERS_FILE.pre-wireceptor"
APP_DIR="/opt/wireceptor"
SUDOERS_RULES=(
  "$USER ALL=(ALL) NOPASSWD: /bin/systemctl start wg-quick@$WG_INTERFACE"
  "$USER ALL=(ALL) NOPASSWD: /bin/systemctl stop wg-quick@$WG_INTERFACE"
)

echo "Backing up current sudoers file to $BACKUP_FILE"
sudo cp $SUDOERS_FILE $BACKUP_FILE

if [ $? -ne 0 ]; then
  echo "Failed to backup sudoers file. Exiting."
  exit 1
fi

rule_exists() {
  sudo grep -Fxq "$1" $SUDOERS_FILE
}

echo "Adding rules to allow $USER to manage WireGuard without a password"
for rule in "${SUDOERS_RULES[@]}"; do
  if rule_exists "$rule"; then
    echo "Rule already exists: $rule"
  else
    echo "$rule" | sudo tee -a $SUDOERS_FILE
  fi
done

sudo visudo -c

if [ $? -eq 0 ]; then
  echo "Sudoers file updated successfully. No password will be required for WireGuard management commands."
else
  echo "There was an error in the sudoers file. Restoring the backup."
  sudo cp $BACKUP_FILE $SUDOERS_FILE
  sudo visudo -c
  if [ $? -eq 0 ]; then
    echo "Backup restored successfully."
  else
    echo "Failed to restore the backup. Please check the sudoers file manually."
  fi
  exit 1
fi

update_script() {
  local script_path="$1"
  if [ -f "$script_path" ]; then
    while IFS= read -r line; do
      if [[ $line == systemctl* ]]; then
        updated_line=$(echo "$line" | sudo sed 's/^systemctl /sudo systemctl /')
        if [[ $line != "$updated_line" ]]; then
          echo "Updating $script_path"
          sudo sed -i "s|^$line|$updated_line|" "$script_path"
        fi
      fi
    done < "$script_path"
  else
    echo "File $script_path does not exist. Please check the file path."
    exit 1
  fi
}

update_script "$APP_DIR/scripts/start.sh"
update_script "$APP_DIR/scripts/stop.sh"

echo "Updated scripts for managing start/stop."
