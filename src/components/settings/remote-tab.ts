import { remoteSettingsStore, generatePassword, generateApiKey } from '../../state/remote-settings-store';
import type { SettingsTabProvider, SettingsDialogContext } from './types';

export class RemoteTab implements SettingsTabProvider {
  id = 'remote';
  label = 'Remote';

  buildContent(_dialog: SettingsDialogContext): HTMLDivElement {
    const content = document.createElement('div');
    content.className = 'settings-tab-content';

    // Password section
    const pwSection = document.createElement('div');
    pwSection.className = 'settings-section';

    const pwTitle = document.createElement('div');
    pwTitle.className = 'settings-section-title';
    pwTitle.textContent = 'Password';
    pwSection.appendChild(pwTitle);

    const pwDesc = document.createElement('div');
    pwDesc.className = 'settings-description';
    pwDesc.textContent = 'Set a fixed password for phone access. We recommend saving this in a password manager like 1Password.';
    pwSection.appendChild(pwDesc);

    const pwForm = document.createElement('form');
    pwForm.action = '/remote-setup';
    pwForm.method = 'POST';
    pwForm.autocomplete = 'on';
    pwForm.onsubmit = (e) => e.preventDefault();

    const pwUsername = document.createElement('input');
    pwUsername.type = 'text';
    pwUsername.name = 'username';
    pwUsername.autocomplete = 'username';
    pwUsername.value = 'godly-terminal';
    pwUsername.style.display = 'none';
    pwForm.appendChild(pwUsername);

    const pwInputRow = document.createElement('div');
    pwInputRow.className = 'shortcut-row';
    pwInputRow.style.gap = '8px';

    const pwInput = document.createElement('input');
    pwInput.type = 'password';
    pwInput.name = 'password';
    pwInput.id = 'godly-remote-password';
    pwInput.autocomplete = 'new-password';
    pwInput.className = 'notification-preset';
    pwInput.style.flex = '1';
    pwInput.style.fontFamily = "'Cascadia Code', Consolas, monospace";
    pwInput.placeholder = 'Enter or generate a password';
    pwInput.setAttribute('passwordrules', 'minlength: 16; maxlength: 128; required: upper, lower, digit, special;');
    pwInput.value = remoteSettingsStore.getPassword();
    pwInputRow.appendChild(pwInput);

    const pwShowBtn = document.createElement('button');
    pwShowBtn.className = 'dialog-btn dialog-btn-secondary';
    pwShowBtn.textContent = 'Show';
    pwShowBtn.style.fontSize = '11px';
    pwShowBtn.style.padding = '2px 10px';
    pwShowBtn.style.minWidth = '50px';
    pwShowBtn.onclick = () => {
      if (pwInput.type === 'password') {
        pwInput.type = 'text';
        pwShowBtn.textContent = 'Hide';
      } else {
        pwInput.type = 'password';
        pwShowBtn.textContent = 'Show';
      }
    };
    pwInputRow.appendChild(pwShowBtn);
    pwForm.appendChild(pwInputRow);

    const pwButtonRow = document.createElement('div');
    pwButtonRow.className = 'shortcut-row';
    pwButtonRow.style.marginTop = '8px';
    pwButtonRow.style.gap = '8px';

    const generateBtn = document.createElement('button');
    generateBtn.className = 'dialog-btn dialog-btn-secondary';
    generateBtn.textContent = 'Generate Strong Password';
    generateBtn.onclick = () => {
      pwInput.value = generatePassword(100);
      pwInput.type = 'text';
      pwShowBtn.textContent = 'Hide';
    };
    pwButtonRow.appendChild(generateBtn);

    const copyBtn = document.createElement('button');
    copyBtn.className = 'dialog-btn dialog-btn-secondary';
    copyBtn.textContent = 'Copy';
    copyBtn.onclick = async () => {
      if (!pwInput.value) return;
      try {
        await navigator.clipboard.writeText(pwInput.value);
        const original = copyBtn.textContent;
        copyBtn.textContent = 'Copied!';
        setTimeout(() => { copyBtn.textContent = original; }, 1500);
      } catch {
        // Clipboard API may not be available
      }
    };
    pwButtonRow.appendChild(copyBtn);

    const pwSaveBtn = document.createElement('button');
    pwSaveBtn.className = 'dialog-btn dialog-btn-primary';
    pwSaveBtn.textContent = 'Save Password';
    pwSaveBtn.onclick = () => {
      remoteSettingsStore.setPassword(pwInput.value);
      const original = pwSaveBtn.textContent;
      pwSaveBtn.textContent = 'Saved!';
      setTimeout(() => { pwSaveBtn.textContent = original; }, 1500);
    };
    pwButtonRow.appendChild(pwSaveBtn);
    pwForm.appendChild(pwButtonRow);

    pwSection.appendChild(pwForm);
    content.appendChild(pwSection);

    // Server section
    const serverSection = document.createElement('div');
    serverSection.className = 'settings-section';

    const serverTitle = document.createElement('div');
    serverTitle.className = 'settings-section-title';
    serverTitle.textContent = 'Server';
    serverSection.appendChild(serverTitle);

    const portRow = document.createElement('div');
    portRow.className = 'shortcut-row';

    const portLabel = document.createElement('span');
    portLabel.className = 'shortcut-label';
    portLabel.textContent = 'Port';
    portRow.appendChild(portLabel);

    const portInput = document.createElement('input');
    portInput.type = 'number';
    portInput.className = 'notification-preset';
    portInput.style.width = '100px';
    portInput.min = '1024';
    portInput.max = '65535';
    portInput.value = String(remoteSettingsStore.getPort());
    portInput.onchange = () => {
      const port = parseInt(portInput.value);
      if (port >= 1024 && port <= 65535) {
        remoteSettingsStore.setPort(port);
      }
    };
    portRow.appendChild(portInput);
    serverSection.appendChild(portRow);

    const autoStartRow = document.createElement('div');
    autoStartRow.className = 'shortcut-row';

    const autoStartLabel = document.createElement('span');
    autoStartLabel.className = 'shortcut-label';
    autoStartLabel.textContent = 'Start remote server on launch';
    autoStartRow.appendChild(autoStartLabel);

    const autoStartCheckbox = document.createElement('input');
    autoStartCheckbox.type = 'checkbox';
    autoStartCheckbox.className = 'notification-checkbox';
    autoStartCheckbox.checked = remoteSettingsStore.getAutoStart();
    autoStartCheckbox.onchange = () => {
      remoteSettingsStore.setAutoStart(autoStartCheckbox.checked);
    };
    autoStartRow.appendChild(autoStartCheckbox);
    serverSection.appendChild(autoStartRow);

    // API Key row
    const apiKeyRow = document.createElement('div');
    apiKeyRow.className = 'shortcut-row';

    const apiKeyLabel = document.createElement('span');
    apiKeyLabel.className = 'shortcut-label';
    apiKeyLabel.textContent = 'API Key';
    apiKeyRow.appendChild(apiKeyLabel);

    const apiKeyDisplay = document.createElement('span');
    apiKeyDisplay.className = 'shortcut-binding';
    apiKeyDisplay.style.fontFamily = "'Cascadia Code', Consolas, monospace";
    apiKeyDisplay.style.fontSize = '11px';
    const storedApiKey = remoteSettingsStore.getApiKey();
    apiKeyDisplay.textContent = storedApiKey ? storedApiKey.slice(0, 8) + '...' : '(not set)';
    apiKeyRow.appendChild(apiKeyDisplay);

    const apiKeyGenBtn = document.createElement('button');
    apiKeyGenBtn.className = 'dialog-btn dialog-btn-secondary';
    apiKeyGenBtn.textContent = storedApiKey ? 'Regenerate' : 'Generate';
    apiKeyGenBtn.style.fontSize = '11px';
    apiKeyGenBtn.style.padding = '2px 10px';
    apiKeyGenBtn.onclick = () => {
      const newKey = generateApiKey();
      remoteSettingsStore.setApiKey(newKey);
      apiKeyDisplay.textContent = newKey.slice(0, 8) + '...';
      apiKeyGenBtn.textContent = 'Regenerate';
    };
    apiKeyRow.appendChild(apiKeyGenBtn);
    serverSection.appendChild(apiKeyRow);

    content.appendChild(serverSection);

    // Connection section
    const connSection = document.createElement('div');
    connSection.className = 'settings-section';

    const connTitle = document.createElement('div');
    connTitle.className = 'settings-section-title';
    connTitle.textContent = 'Connection';
    connSection.appendChild(connTitle);

    const connDesc = document.createElement('div');
    connDesc.className = 'settings-description';
    connDesc.textContent = 'Run setup-phone.ps1 to start the remote server with an ngrok tunnel. The script will use your saved password, port, and API key.';
    connSection.appendChild(connDesc);

    const connInstructions = document.createElement('div');
    connInstructions.className = 'settings-description';
    connInstructions.style.fontFamily = "'Cascadia Code', Consolas, monospace";
    connInstructions.style.fontSize = '12px';
    connInstructions.style.marginTop = '8px';
    connInstructions.textContent = 'pwsh scripts/setup-phone.ps1';
    connSection.appendChild(connInstructions);

    content.appendChild(connSection);

    return content;
  }
}
