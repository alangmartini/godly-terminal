import { llmHasApiKey, llmGenerateBranchName } from '../plugins/smollm2/llm-service';
import { aiToolsSettingsStore } from '../state/ai-tools-settings-store';
import { quickClaudeSettingsStore } from '../state/quick-claude-settings-store';

/**
 * Show a prompt dialog for entering a custom worktree branch name.
 * Returns the user's input (empty string = auto-generate), or null if cancelled.
 */
export function showWorktreeNamePrompt(customTitle?: string): Promise<string | null> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = customTitle || 'New Worktree Branch';
    dialog.appendChild(title);

    // Description input for AI suggestion
    const descInput = document.createElement('input');
    descInput.type = 'text';
    descInput.className = 'dialog-input';
    descInput.placeholder = 'Describe the task (for AI branch name)';
    descInput.style.marginBottom = '4px';

    const inputRow = document.createElement('div');
    inputRow.style.display = 'flex';
    inputRow.style.gap = '8px';
    inputRow.style.alignItems = 'center';

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'dialog-input';
    input.placeholder = 'Branch name (Enter for auto-generated)';
    input.style.flex = '1';
    inputRow.appendChild(input);

    const aiBtn = document.createElement('button');
    aiBtn.className = 'dialog-btn dialog-btn-secondary';
    aiBtn.textContent = 'AI Suggest';
    aiBtn.style.cssText = 'font-size: 11px; padding: 4px 10px; white-space: nowrap; display: none;';
    aiBtn.onclick = async () => {
      const desc = descInput.value.trim();
      if (!desc) {
        descInput.focus();
        return;
      }
      aiBtn.disabled = true;
      aiBtn.textContent = 'Thinking...';
      try {
        const name = await llmGenerateBranchName(desc);
        input.value = name;
      } catch (e) {
        console.warn('[Dialogs] AI suggest failed:', e);
      } finally {
        aiBtn.disabled = false;
        aiBtn.textContent = 'AI Suggest';
      }
    };
    inputRow.appendChild(aiBtn);

    // Check if model is ready and show AI features
    llmHasApiKey().then(hasKey => {
      if (hasKey) {
        descInput.style.display = '';
        aiBtn.style.display = '';
      }
    }).catch(() => {});

    descInput.style.display = 'none';
    dialog.appendChild(descInput);
    dialog.appendChild(inputRow);

    const buttons = document.createElement('div');
    buttons.className = 'dialog-buttons';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn dialog-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    buttons.appendChild(cancelBtn);

    const okBtn = document.createElement('button');
    okBtn.className = 'dialog-btn dialog-btn-primary';
    okBtn.textContent = 'Create';
    buttons.appendChild(okBtn);

    dialog.appendChild(buttons);
    overlay.appendChild(dialog);

    const close = () => overlay.remove();

    cancelBtn.onclick = () => {
      close();
      resolve(null);
    };

    okBtn.onclick = () => {
      close();
      resolve(input.value.trim());
    };

    input.onkeydown = (e) => {
      if (e.key === 'Enter') {
        close();
        resolve(input.value.trim());
      }
      if (e.key === 'Escape') {
        close();
        resolve(null);
      }
    };

    descInput.onkeydown = (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        aiBtn.click();
      }
      if (e.key === 'Escape') {
        close();
        resolve(null);
      }
    };

    overlay.onclick = (e) => {
      if (e.target === overlay) {
        close();
        resolve(null);
      }
    };

    document.body.appendChild(overlay);
    input.focus();
  });
}

/**
 * Show a prompt dialog for entering a Figma file URL.
 * Returns the URL string, or null if cancelled.
 */
export function showFigmaUrlPrompt(): Promise<string | null> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = 'Open Figma Design';
    dialog.appendChild(title);

    const hint = document.createElement('div');
    hint.style.cssText = 'font-size: 12px; color: var(--text-secondary); margin-bottom: 12px;';
    hint.textContent = 'Paste a Figma file URL (e.g. https://figma.com/design/...)';
    dialog.appendChild(hint);

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'dialog-input';
    input.placeholder = 'https://figma.com/design/...';
    dialog.appendChild(input);

    const buttons = document.createElement('div');
    buttons.className = 'dialog-buttons';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn dialog-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    buttons.appendChild(cancelBtn);

    const okBtn = document.createElement('button');
    okBtn.className = 'dialog-btn dialog-btn-primary';
    okBtn.textContent = 'Open';
    buttons.appendChild(okBtn);

    dialog.appendChild(buttons);
    overlay.appendChild(dialog);

    const close = () => overlay.remove();

    const submit = () => {
      const url = input.value.trim();
      close();
      if (url && url.includes('figma.com')) {
        resolve(url);
      } else if (url) {
        // Not a valid Figma URL
        resolve(null);
      } else {
        resolve(null);
      }
    };

    cancelBtn.onclick = () => {
      close();
      resolve(null);
    };

    okBtn.onclick = submit;

    input.onkeydown = (e) => {
      if (e.key === 'Enter') submit();
      if (e.key === 'Escape') {
        close();
        resolve(null);
      }
    };

    overlay.onclick = (e) => {
      if (e.target === overlay) {
        close();
        resolve(null);
      }
    };

    document.body.appendChild(overlay);
    input.focus();
  });
}

/**
 * Quick Claude dialog: capture an idea to dispatch to a new Claude Code session.
 * Returns { prompt, branchName? } or null if cancelled.
 */
export interface QuickClaudeInput {
  prompt: string;
  branchName?: string;
  workspaceId: string;
  noWorktree?: boolean;
  aiTool?: string;
  presetId?: string;
}

export interface QuickClaudeOptions {
  workspaces: { id: string; name: string; folderPath: string; aiToolMode?: string }[];
  activeWorkspaceId: string;
}

const QUICK_CLAUDE_WORKSPACE_KEY = 'quick-claude-last-workspace';
const QUICK_CLAUDE_NO_WORKTREE_KEY = 'quick-claude-no-worktree';
const QUICK_CLAUDE_AUTO_SUGGEST_KEY = 'quick-claude-auto-suggest';
const QUICK_CLAUDE_AI_TOOL_KEY = 'quick-claude-ai-tool';
const QUICK_CLAUDE_PRESET_KEY = 'quick-claude-preset';

const IMAGE_EXTENSIONS = new Set([
  '.png', '.jpg', '.jpeg', '.gif', '.bmp', '.webp', '.svg', '.tiff', '.tif', '.ico',
]);

function isImagePath(path: string): boolean {
  const ext = path.slice(path.lastIndexOf('.')).toLowerCase();
  return IMAGE_EXTENSIONS.has(ext);
}

export function showQuickClaudeDialog(options: QuickClaudeOptions): Promise<QuickClaudeInput | null> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = 'Quick Claude';
    dialog.appendChild(title);

    const hint = document.createElement('div');
    hint.style.cssText = 'font-size: 12px; color: var(--text-secondary); margin-bottom: 8px;';
    hint.textContent = 'Ctrl+Enter to launch \u00b7 Shift+V voice \u00b7 Shift+B AI suggest \u00b7 Escape to cancel';
    dialog.appendChild(hint);

    // Step indicators
    const stepsRow = document.createElement('div');
    stepsRow.className = 'qc-steps';
    const step1 = document.createElement('span');
    step1.className = 'qc-step qc-step-active';
    step1.textContent = '\u2460 Workspace';
    const arrow1 = document.createElement('span');
    arrow1.className = 'qc-step-arrow';
    arrow1.textContent = '\u2192';
    const step2 = document.createElement('span');
    step2.className = 'qc-step';
    step2.textContent = '\u2461 Prompt';
    const arrow2 = document.createElement('span');
    arrow2.className = 'qc-step-arrow';
    arrow2.textContent = '\u2192';
    const step3 = document.createElement('span');
    step3.className = 'qc-step';
    step3.textContent = '\u2462 Launch';
    stepsRow.append(step1, arrow1, step2, arrow2, step3);
    dialog.appendChild(stepsRow);

    function setActiveStep(n: number) {
      step1.classList.toggle('qc-step-active', n === 1);
      step2.classList.toggle('qc-step-active', n === 2);
      step3.classList.toggle('qc-step-active', n === 3);
    }

    const workspaceSelect = document.createElement('select');
    workspaceSelect.className = 'dialog-input';
    workspaceSelect.style.marginBottom = '8px';
    for (const ws of options.workspaces) {
      const opt = document.createElement('option');
      opt.value = ws.id;
      opt.textContent = ws.name;
      workspaceSelect.appendChild(opt);
    }
    const savedId = localStorage.getItem(QUICK_CLAUDE_WORKSPACE_KEY);
    const validSaved = savedId && options.workspaces.some(ws => ws.id === savedId);
    workspaceSelect.value = validSaved ? savedId : options.activeWorkspaceId;
    workspaceSelect.tabIndex = 1;
    workspaceSelect.addEventListener('focus', () => setActiveStep(1));
    dialog.appendChild(workspaceSelect);

    // -- Preset selector --
    const presetSelect = document.createElement('select');
    presetSelect.className = 'dialog-input qc-preset-select';
    presetSelect.style.marginBottom = '8px';

    const agentSummary = document.createElement('div');
    agentSummary.className = 'qc-agent-summary';
    agentSummary.style.display = 'none';

    function populatePresets() {
      presetSelect.innerHTML = '';
      const presets = quickClaudeSettingsStore.getPresets();
      for (const p of presets) {
        const opt = document.createElement('option');
        opt.value = p.id;
        opt.textContent = p.name + (p.isDefault ? ' (Default)' : '');
        presetSelect.appendChild(opt);
      }
      const customOpt = document.createElement('option');
      customOpt.value = '__custom__';
      customOpt.textContent = 'Custom...';
      presetSelect.appendChild(customOpt);
    }
    populatePresets();

    const savedPreset = localStorage.getItem(QUICK_CLAUDE_PRESET_KEY);
    const presetIds = quickClaudeSettingsStore.getPresets().map(p => p.id);
    if (savedPreset && presetIds.includes(savedPreset)) {
      presetSelect.value = savedPreset;
    } else {
      const defaultPreset = quickClaudeSettingsStore.getDefaultPreset();
      presetSelect.value = defaultPreset?.id ?? '__custom__';
    }

    dialog.appendChild(presetSelect);

    // -- AI tool selector --
    const aiToolSelect = document.createElement('select');
    aiToolSelect.className = 'dialog-input ai-tool-mode-select';
    aiToolSelect.dataset.testid = 'ai-tool-mode';
    aiToolSelect.style.marginBottom = '8px';
    for (const tool of aiToolsSettingsStore.getAllToolOptions()) {
      const opt = document.createElement('option');
      opt.value = tool.id;
      opt.textContent = tool.name;
      aiToolSelect.appendChild(opt);
    }

    // Default from selected workspace's aiToolMode
    const getWsAiMode = (wsId: string) => {
      const ws = options.workspaces.find(w => w.id === wsId);
      const mode = ws?.aiToolMode;
      if (mode === 'both') return 'both';
      return mode === 'codex' ? 'codex' : 'claude';
    };
    const savedAiTool = localStorage.getItem(QUICK_CLAUDE_AI_TOOL_KEY);
    const validAiTools = ['claude', 'codex', 'both'];
    if (savedAiTool && validAiTools.includes(savedAiTool)) {
      aiToolSelect.value = savedAiTool;
    } else {
      aiToolSelect.value = getWsAiMode(workspaceSelect.value);
    }
    workspaceSelect.addEventListener('change', () => {
      if (!localStorage.getItem(QUICK_CLAUDE_AI_TOOL_KEY)) {
        aiToolSelect.value = getWsAiMode(workspaceSelect.value);
      }
    });

    function updatePresetUI() {
      const isCustom = presetSelect.value === '__custom__';
      aiToolSelect.style.display = isCustom ? '' : 'none';
      if (!isCustom) {
        const preset = quickClaudeSettingsStore.getPreset(presetSelect.value);
        if (preset) {
          const names = preset.agents.map(a => a.label).join(', ');
          const layoutLabel = preset.layout === 'single' ? '' : ` \u00b7 ${preset.layout}`;
          agentSummary.textContent = `${names}${layoutLabel}`;
          agentSummary.style.display = '';
        } else {
          agentSummary.style.display = 'none';
        }
      } else {
        agentSummary.style.display = 'none';
      }
    }

    presetSelect.addEventListener('change', updatePresetUI);
    updatePresetUI();

    dialog.appendChild(aiToolSelect);
    dialog.appendChild(agentSummary);

    // -- Prompt textarea with skill dropdown wrapper --
    const promptWrapper = document.createElement('div');
    promptWrapper.style.position = 'relative';

    const promptArea = document.createElement('textarea');
    promptArea.className = 'dialog-input';
    promptArea.placeholder = 'Describe your idea... (/ for skills, @ for files)';
    promptArea.rows = 4;
    promptArea.style.cssText = 'resize: vertical; min-height: 80px; font-family: inherit; font-size: 13px;';
    promptArea.tabIndex = 2;
    promptArea.addEventListener('focus', () => setActiveStep(2));
    promptWrapper.appendChild(promptArea);

    const skillDropdown = document.createElement('div');
    skillDropdown.className = 'skill-dropdown';
    skillDropdown.style.display = 'none';
    promptWrapper.appendChild(skillDropdown);

    const fileDropdown = document.createElement('div');
    fileDropdown.className = 'file-dropdown';
    fileDropdown.style.display = 'none';
    promptWrapper.appendChild(fileDropdown);

    dialog.appendChild(promptWrapper);

    // -- Skill autocomplete state --
    interface SkillInfo { name: string; description: string; usage: string; source: string }
    const skillCache = new Map<string, SkillInfo[]>();
    let activeSkills: SkillInfo[] = [];
    let activeIndex = -1;
    let dropdownVisible = false;

    async function fetchSkills(workspaceId: string): Promise<SkillInfo[]> {
      if (skillCache.has(workspaceId)) return skillCache.get(workspaceId)!;
      const ws = options.workspaces.find(w => w.id === workspaceId);
      if (!ws) return [];
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const skills = await invoke<SkillInfo[]>('list_skills', { projectPath: ws.folderPath });
        skillCache.set(workspaceId, skills);
        return skills;
      } catch {
        return [];
      }
    }

    function renderDropdown(skills: SkillInfo[], highlightIndex: number) {
      skillDropdown.innerHTML = '';
      if (skills.length === 0) {
        hideDropdown();
        return;
      }
      skills.forEach((skill, i) => {
        const item = document.createElement('div');
        item.className = 'skill-item' + (i === highlightIndex ? ' skill-item-active' : '');
        const nameEl = document.createElement('div');
        nameEl.className = 'skill-item-name';
        nameEl.textContent = '/' + skill.name;
        const descEl = document.createElement('div');
        descEl.className = 'skill-item-desc';
        descEl.textContent = skill.description;
        item.appendChild(nameEl);
        item.appendChild(descEl);
        item.addEventListener('mousedown', (e) => {
          e.preventDefault();
          selectSkill(skill);
        });
        item.addEventListener('mouseenter', () => {
          activeIndex = i;
          updateHighlight();
        });
        skillDropdown.appendChild(item);
      });
      skillDropdown.style.display = '';
      dropdownVisible = true;
    }

    function updateHighlight() {
      const items = skillDropdown.querySelectorAll('.skill-item');
      items.forEach((el, i) => {
        el.classList.toggle('skill-item-active', i === activeIndex);
        if (i === activeIndex) el.scrollIntoView({ block: 'nearest' });
      });
    }

    function hideDropdown() {
      skillDropdown.style.display = 'none';
      dropdownVisible = false;
      activeIndex = -1;
      activeSkills = [];
    }

    function selectSkill(skill: SkillInfo) {
      const val = promptArea.value;
      const cursor = promptArea.selectionStart;
      const before = val.slice(0, cursor);
      const slashIdx = before.lastIndexOf('/');
      if (slashIdx >= 0) {
        const replacement = skill.usage || ('/' + skill.name);
        promptArea.value = val.slice(0, slashIdx) + replacement + ' ' + val.slice(cursor);
        const newPos = slashIdx + replacement.length + 1;
        promptArea.setSelectionRange(newPos, newPos);
      }
      hideDropdown();
      promptArea.focus();
    }

    async function refreshSkillDropdown() {
      const val = promptArea.value;
      const cursor = promptArea.selectionStart;
      const before = val.slice(0, cursor);
      const match = before.match(/(^|[\s\n])\/([\w-]*)$/);
      if (!match) {
        hideDropdown();
        return;
      }
      const query = match[2].toLowerCase();
      const skills = await fetchSkills(workspaceSelect.value);
      const filtered = query
        ? skills.filter(s => s.name.toLowerCase().includes(query))
        : skills;
      activeSkills = filtered;
      activeIndex = filtered.length > 0 ? 0 : -1;
      renderDropdown(filtered, activeIndex);
    }

    promptArea.addEventListener('input', () => {
      refreshSkillDropdown();
      refreshFileDropdown();
    });

    workspaceSelect.addEventListener('change', () => {
      const before = promptArea.value.slice(0, promptArea.selectionStart);
      if (dropdownVisible || before.match(/(^|[\s\n])\/([\w-]*)$/)) {
        refreshSkillDropdown();
      }
      // Clear file cache and refresh if file dropdown is active
      dirCache.clear();
      if (fileDropdownVisible) {
        refreshFileDropdown();
      }
    });

    // -- File autocomplete state --
    interface DirEntryInfo { name: string; is_dir: boolean }
    const dirCache = new Map<string, DirEntryInfo[]>();
    let activeFiles: DirEntryInfo[] = [];
    let fileActiveIndex = -1;
    let fileDropdownVisible = false;

    async function fetchDirEntries(dirPath: string): Promise<DirEntryInfo[]> {
      const wsId = workspaceSelect.value;
      const cacheKey = `${wsId}:${dirPath}`;
      if (dirCache.has(cacheKey)) return dirCache.get(cacheKey)!;
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const entries = await invoke<DirEntryInfo[]>('list_directory', { path: dirPath });
        dirCache.set(cacheKey, entries);
        return entries;
      } catch {
        return [];
      }
    }

    function renderFileDropdown(entries: DirEntryInfo[], highlightIndex: number) {
      fileDropdown.innerHTML = '';
      if (entries.length === 0) {
        hideFileDropdown();
        return;
      }
      entries.forEach((entry, i) => {
        const item = document.createElement('div');
        item.className = 'file-item' + (i === highlightIndex ? ' file-item-active' : '');
        const icon = document.createElement('span');
        icon.className = 'file-item-icon';
        icon.textContent = entry.is_dir ? '\uD83D\uDCC1' : '\uD83D\uDCC4';
        const nameEl = document.createElement('span');
        nameEl.className = 'file-item-name';
        nameEl.textContent = entry.name + (entry.is_dir ? '/' : '');
        item.appendChild(icon);
        item.appendChild(nameEl);
        item.addEventListener('mousedown', (e) => {
          e.preventDefault();
          selectFile(entry);
        });
        item.addEventListener('mouseenter', () => {
          fileActiveIndex = i;
          updateFileHighlight();
        });
        fileDropdown.appendChild(item);
      });
      fileDropdown.style.display = '';
      fileDropdownVisible = true;
    }

    function updateFileHighlight() {
      const items = fileDropdown.querySelectorAll('.file-item');
      items.forEach((el, i) => {
        el.classList.toggle('file-item-active', i === fileActiveIndex);
        if (i === fileActiveIndex) el.scrollIntoView?.({ block: 'nearest' });
      });
    }

    function hideFileDropdown() {
      fileDropdown.style.display = 'none';
      fileDropdownVisible = false;
      fileActiveIndex = -1;
      activeFiles = [];
    }

    function selectFile(entry: DirEntryInfo) {
      const val = promptArea.value;
      const cursor = promptArea.selectionStart;
      const before = val.slice(0, cursor);
      const atMatch = before.match(/(^|[\s\n])@([\w./_-]*)$/);
      if (!atMatch) { hideFileDropdown(); return; }
      const atStart = before.lastIndexOf('@');
      const currentPath = atMatch[2];
      const lastSlash = currentPath.lastIndexOf('/');
      const dirPrefix = lastSlash >= 0 ? currentPath.slice(0, lastSlash + 1) : '';

      if (entry.is_dir) {
        const newPath = dirPrefix + entry.name + '/';
        promptArea.value = val.slice(0, atStart) + '@' + newPath + val.slice(cursor);
        const newPos = atStart + 1 + newPath.length;
        promptArea.setSelectionRange(newPos, newPos);
        promptArea.focus();
        refreshFileDropdown();
      } else {
        const fullPath = dirPrefix + entry.name;
        promptArea.value = val.slice(0, atStart) + '@' + fullPath + ' ' + val.slice(cursor);
        const newPos = atStart + 1 + fullPath.length + 1;
        promptArea.setSelectionRange(newPos, newPos);
        hideFileDropdown();
        promptArea.focus();
      }
    }

    async function refreshFileDropdown() {
      const val = promptArea.value;
      const cursor = promptArea.selectionStart;
      const before = val.slice(0, cursor);
      const atMatch = before.match(/(^|[\s\n])@([\w./_-]*)$/);
      if (!atMatch) {
        hideFileDropdown();
        return;
      }
      const currentPath = atMatch[2];
      const lastSlash = currentPath.lastIndexOf('/');
      const dirPart = lastSlash >= 0 ? currentPath.slice(0, lastSlash) : '';
      const filterPart = lastSlash >= 0 ? currentPath.slice(lastSlash + 1) : currentPath;

      const ws = options.workspaces.find(w => w.id === workspaceSelect.value);
      if (!ws) { hideFileDropdown(); return; }

      const fullDirPath = dirPart ? `${ws.folderPath}/${dirPart}` : ws.folderPath;
      const entries = await fetchDirEntries(fullDirPath);
      const filtered = filterPart
        ? entries.filter(e => e.name.toLowerCase().includes(filterPart.toLowerCase()))
        : entries;

      activeFiles = filtered;
      fileActiveIndex = filtered.length > 0 ? 0 : -1;
      renderFileDropdown(filtered, fileActiveIndex);
    }

    // -- Image attachments (drag-and-drop) --
    const attachedImages: string[] = [];

    const attachContainer = document.createElement('div');
    attachContainer.className = 'quick-claude-attachments';
    attachContainer.style.display = 'none';
    dialog.appendChild(attachContainer);

    function addImage(path: string) {
      if (attachedImages.includes(path)) return;
      attachedImages.push(path);
      renderAttachments();
    }

    function removeImage(path: string) {
      const idx = attachedImages.indexOf(path);
      if (idx >= 0) {
        attachedImages.splice(idx, 1);
        renderAttachments();
      }
    }

    function renderAttachments() {
      attachContainer.innerHTML = '';
      if (attachedImages.length === 0) {
        attachContainer.style.display = 'none';
        return;
      }
      attachContainer.style.display = '';
      for (const imgPath of attachedImages) {
        const chip = document.createElement('div');
        chip.className = 'quick-claude-image-chip';

        const icon = document.createElement('span');
        icon.className = 'quick-claude-image-chip-icon';
        icon.textContent = '\uD83D\uDDBC'; // framed picture emoji
        chip.appendChild(icon);

        const nameEl = document.createElement('span');
        nameEl.className = 'quick-claude-image-chip-name';
        const fileName = imgPath.split(/[\\/]/).pop() || imgPath;
        nameEl.textContent = fileName;
        nameEl.title = imgPath;
        chip.appendChild(nameEl);

        const removeBtn = document.createElement('span');
        removeBtn.className = 'quick-claude-image-chip-remove';
        removeBtn.textContent = '\u00d7';
        removeBtn.title = 'Remove';
        removeBtn.onclick = () => removeImage(imgPath);
        chip.appendChild(removeBtn);

        attachContainer.appendChild(chip);
      }
    }

    // Register Tauri drag-drop listener for images while dialog is open
    let unlistenDragDrop: (() => void) | null = null;
    (async () => {
      try {
        const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');
        unlistenDragDrop = await getCurrentWebviewWindow().onDragDropEvent((event) => {
          if (event.payload.type === 'enter') {
            dialog.classList.add('quick-claude-drag-over');
          } else if (event.payload.type === 'leave') {
            dialog.classList.remove('quick-claude-drag-over');
          } else if (event.payload.type === 'drop') {
            dialog.classList.remove('quick-claude-drag-over');
            const paths: string[] = (event.payload as { paths?: string[] }).paths || [];
            for (const p of paths) {
              if (isImagePath(p)) {
                addImage(p);
              }
            }
          }
        });
      } catch (err) {
        console.warn('[QuickClaude] Failed to register drag-drop listener:', err);
      }
    })();

    const branchRow = document.createElement('div');
    branchRow.style.cssText = 'display: flex; gap: 8px; align-items: center; margin-top: 8px;';

    const branchInput = document.createElement('input');
    branchInput.type = 'text';
    branchInput.className = 'dialog-input';
    branchInput.placeholder = 'Branch name (optional, auto-generated if empty)';
    branchInput.style.flex = '1';
    branchInput.tabIndex = -1;
    branchRow.appendChild(branchInput);

    const branchAiBtn = document.createElement('button');
    branchAiBtn.className = 'dialog-btn dialog-btn-secondary';
    branchAiBtn.textContent = 'AI Suggest';
    branchAiBtn.style.cssText = 'font-size: 11px; padding: 4px 10px; white-space: nowrap; display: none;';
    branchAiBtn.tabIndex = -1;
    branchAiBtn.onclick = async () => {
      const desc = promptArea.value.trim();
      if (!desc) {
        promptArea.focus();
        return;
      }
      branchAiBtn.disabled = true;
      branchAiBtn.textContent = 'Thinking...';
      try {
        const name = await llmGenerateBranchName(desc);
        branchInput.value = name;
      } catch (e) {
        console.warn('[Dialogs] AI suggest failed:', e);
      } finally {
        branchAiBtn.disabled = false;
        branchAiBtn.textContent = 'AI Suggest';
      }
    };
    branchRow.appendChild(branchAiBtn);

    // Show AI button if model is ready
    llmHasApiKey().then(hasKey => {
      if (hasKey) {
        branchAiBtn.style.display = '';
      }
    }).catch(() => {});

    dialog.appendChild(branchRow);

    // -- No worktree checkbox --
    const worktreeRow = document.createElement('label');
    worktreeRow.style.cssText = 'display: flex; align-items: center; gap: 6px; margin-top: 8px; font-size: 12px; color: var(--text-secondary); cursor: pointer; user-select: none;';

    const noWorktreeCheckbox = document.createElement('input');
    noWorktreeCheckbox.type = 'checkbox';
    noWorktreeCheckbox.style.margin = '0';
    noWorktreeCheckbox.tabIndex = -1;
    const savedNoWorktree = localStorage.getItem(QUICK_CLAUDE_NO_WORKTREE_KEY) === 'true';
    noWorktreeCheckbox.checked = savedNoWorktree;
    worktreeRow.appendChild(noWorktreeCheckbox);
    worktreeRow.append('Open in main branch (no worktree)');

    // Apply initial state if restored from localStorage
    if (savedNoWorktree) {
      branchInput.disabled = true;
      branchAiBtn.disabled = true;
      branchInput.style.opacity = '0.5';
    }

    noWorktreeCheckbox.addEventListener('change', () => {
      const disabled = noWorktreeCheckbox.checked;
      branchInput.disabled = disabled;
      branchAiBtn.disabled = disabled;
      branchInput.style.opacity = disabled ? '0.5' : '1';
    });

    dialog.appendChild(worktreeRow);

    // -- Auto AI suggest checkbox --
    const autoSuggestRow = document.createElement('label');
    autoSuggestRow.style.cssText = 'display: flex; align-items: center; gap: 6px; margin-top: 4px; font-size: 12px; color: var(--text-secondary); cursor: pointer; user-select: none;';

    const autoSuggestCheckbox = document.createElement('input');
    autoSuggestCheckbox.type = 'checkbox';
    autoSuggestCheckbox.style.margin = '0';
    autoSuggestCheckbox.tabIndex = -1;
    autoSuggestCheckbox.checked = localStorage.getItem(QUICK_CLAUDE_AUTO_SUGGEST_KEY) === 'true';
    autoSuggestRow.appendChild(autoSuggestCheckbox);
    autoSuggestRow.append('Auto-suggest branch name when leaving prompt');

    autoSuggestCheckbox.addEventListener('change', () => {
      localStorage.setItem(QUICK_CLAUDE_AUTO_SUGGEST_KEY, String(autoSuggestCheckbox.checked));
    });

    promptArea.addEventListener('blur', () => {
      if (
        autoSuggestCheckbox.checked &&
        promptArea.value.trim() &&
        !branchInput.value &&
        !noWorktreeCheckbox.checked &&
        branchAiBtn.style.display !== 'none'
      ) {
        branchAiBtn.click();
      }
    });

    dialog.appendChild(autoSuggestRow);

    const buttons = document.createElement('div');
    buttons.className = 'dialog-buttons';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn dialog-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    cancelBtn.tabIndex = -1;
    buttons.appendChild(cancelBtn);

    // Voice input button for dictation
    const voiceBtn = document.createElement('button');
    voiceBtn.className = 'dialog-btn dialog-btn-secondary quick-claude-voice-btn';
    voiceBtn.tabIndex = -1;
    voiceBtn.textContent = 'Voice';
    voiceBtn.title = 'Dictate with voice';
    voiceBtn.addEventListener('click', async () => {
      try {
        const { whisperGetStatus, whisperStartRecording, whisperStopRecording } = await import('../plugins/voice/whisper-service');
        const status = await whisperGetStatus();
        if (status.state === 'idle') {
          await whisperStartRecording();
          voiceBtn.textContent = 'Stop';
          voiceBtn.classList.add('voice-recording');
        } else if (status.state === 'recording') {
          voiceBtn.textContent = '...';
          voiceBtn.classList.remove('voice-recording');
          const result = await whisperStopRecording();
          voiceBtn.textContent = 'Voice';
          if (result.text) {
            promptArea.value += (promptArea.value ? ' ' : '') + result.text;
            promptArea.dispatchEvent(new Event('input'));
          }
        }
      } catch (err) {
        voiceBtn.textContent = 'Voice';
        voiceBtn.classList.remove('voice-recording');
        console.error('Voice input failed:', err);
      }
    });
    buttons.appendChild(voiceBtn);

    const okBtn = document.createElement('button');
    okBtn.className = 'dialog-btn dialog-btn-primary';
    okBtn.textContent = 'Launch';
    okBtn.tabIndex = 3;
    okBtn.addEventListener('focus', () => setActiveStep(3));
    buttons.appendChild(okBtn);

    dialog.appendChild(buttons);
    overlay.appendChild(dialog);

    const close = () => {
      if (unlistenDragDrop) unlistenDragDrop();
      overlay.remove();
    };

    const submit = () => {
      const promptText = promptArea.value.trim();
      if (!promptText && attachedImages.length === 0) return;
      localStorage.setItem(QUICK_CLAUDE_WORKSPACE_KEY, workspaceSelect.value);
      localStorage.setItem(QUICK_CLAUDE_NO_WORKTREE_KEY, String(noWorktreeCheckbox.checked));
      localStorage.setItem(QUICK_CLAUDE_AI_TOOL_KEY, aiToolSelect.value);
      localStorage.setItem(QUICK_CLAUDE_PRESET_KEY, presetSelect.value);

      // Prepend image paths to the prompt so Claude Code auto-loads them
      let prompt = promptText;
      if (attachedImages.length > 0) {
        const quotedPaths = attachedImages.map(p => p.includes(' ') ? `"${p}"` : p);
        const imagePrefix = quotedPaths.join(' ');
        prompt = prompt ? `${imagePrefix} ${prompt}` : imagePrefix;
      }

      const selectedPresetId = presetSelect.value !== '__custom__' ? presetSelect.value : undefined;

      close();
      resolve({
        prompt,
        branchName: noWorktreeCheckbox.checked ? undefined : (branchInput.value.trim() || undefined),
        workspaceId: workspaceSelect.value,
        noWorktree: noWorktreeCheckbox.checked || undefined,
        aiTool: aiToolSelect.value,
        presetId: selectedPresetId,
      });
    };

    cancelBtn.onclick = () => { close(); resolve(null); };
    okBtn.onclick = submit;

    workspaceSelect.onkeydown = (e) => {
      if (e.key === 'Enter' && e.ctrlKey) { e.preventDefault(); submit(); }
      if (e.key === 'Escape') { close(); resolve(null); }
    };

    okBtn.onkeydown = (e) => {
      if (e.key === 'Enter') { e.preventDefault(); submit(); }
      if (e.key === 'Escape') { close(); resolve(null); }
    };

    promptArea.onkeydown = (e) => {
      if (dropdownVisible) {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          activeIndex = Math.min(activeIndex + 1, activeSkills.length - 1);
          updateHighlight();
          return;
        }
        if (e.key === 'ArrowUp') {
          e.preventDefault();
          activeIndex = Math.max(activeIndex - 1, 0);
          updateHighlight();
          return;
        }
        if (e.key === 'Enter' && !e.ctrlKey) {
          if (activeIndex >= 0 && activeIndex < activeSkills.length) {
            e.preventDefault();
            selectSkill(activeSkills[activeIndex]);
            return;
          }
        }
        if (e.key === 'Escape') {
          e.preventDefault();
          hideDropdown();
          return;
        }
        if (e.key === 'Tab') {
          if (activeIndex >= 0 && activeIndex < activeSkills.length) {
            e.preventDefault();
            selectSkill(activeSkills[activeIndex]);
            return;
          }
        }
      }
      if (fileDropdownVisible) {
        if (e.key === 'ArrowDown') {
          e.preventDefault();
          fileActiveIndex = Math.min(fileActiveIndex + 1, activeFiles.length - 1);
          updateFileHighlight();
          return;
        }
        if (e.key === 'ArrowUp') {
          e.preventDefault();
          fileActiveIndex = Math.max(fileActiveIndex - 1, 0);
          updateFileHighlight();
          return;
        }
        if ((e.key === 'Enter' || e.key === 'Tab') && !e.ctrlKey) {
          if (fileActiveIndex >= 0 && fileActiveIndex < activeFiles.length) {
            e.preventDefault();
            selectFile(activeFiles[fileActiveIndex]);
            return;
          }
        }
        if (e.key === 'Escape') {
          e.preventDefault();
          hideFileDropdown();
          return;
        }
      }
      if (e.key === 'Enter' && e.ctrlKey) { e.preventDefault(); submit(); }
      if (e.key === 'Escape') { close(); resolve(null); }
    };

    branchInput.onkeydown = (e) => {
      if (e.key === 'Enter') { e.preventDefault(); submit(); }
      if (e.key === 'Escape') { close(); resolve(null); }
    };

    overlay.onclick = (e) => {
      if (e.target === overlay) { close(); resolve(null); }
    };

    // Shift+V / Shift+B shortcuts (only when not in a text input)
    dialog.addEventListener('keydown', (e) => {
      const tag = (document.activeElement as HTMLElement)?.tagName;
      if (tag === 'TEXTAREA' || tag === 'INPUT') return;
      if (e.shiftKey && e.key === 'V') {
        e.preventDefault();
        voiceBtn.click();
      }
      if (e.shiftKey && e.key === 'B') {
        e.preventDefault();
        branchAiBtn.click();
      }
    });

    document.body.appendChild(overlay);
    workspaceSelect.focus();
  });
}
