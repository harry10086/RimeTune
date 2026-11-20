import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AppOptionItem, EnvironmentStatus, SystemAppItem } from '../types';
import { Monitor, Plus, Trash2, Shield, CheckSquare, Save, Activity, HelpCircle, ChevronDown, ChevronUp, Sparkles } from 'lucide-react';

interface AppRulesViewProps {
  appOptions: AppOptionItem[];
  env?: EnvironmentStatus | null;
  onChange: (options: AppOptionItem[]) => void;
  onSaveAndDeploy?: () => void;
  saving?: boolean;
}

export const AppRulesView: React.FC<AppRulesViewProps> = ({
  appOptions = [],
  env,
  onChange,
  onSaveAndDeploy,
  saving = false,
}) => {
  const [newAppName, setNewAppName] = useState('');
  const [newAscii, setNewAscii] = useState(true);
  const [newInline, setNewInline] = useState(true);

  // 动态扫描获取的系统桌面应用与运行中进程
  const [systemApps, setSystemApps] = useState<SystemAppItem[]>([]);
  const [scanningApps, setScanningApps] = useState(false);
  const [actionNotice, setActionNotice] = useState<string | null>(null);
  const [showGuide, setShowGuide] = useState(env?.platform === 'linux');

  useEffect(() => {
    setScanningApps(true);
    invoke<SystemAppItem[]>('get_system_applications')
      .then((res) => {
        if (res && res.length > 0) {
          setSystemApps(res);
        }
      })
      .catch((err) => console.warn('Failed to detect system applications:', err))
      .finally(() => setScanningApps(false));
  }, []);

  const showNotice = (msg: string) => {
    setActionNotice(msg);
    setTimeout(() => setActionNotice(null), 4000);
  };

  const handleAdd = () => {
    let name = newAppName.trim();
    if (!name) return;
    // 仅在 Windows 且不含任何后缀时自动补全 .exe
    if (env?.platform === 'windows' && !name.includes('.')) {
      name += '.exe';
    }
    handleAddRule(name, newAscii, newInline);
  };

  const handleAddRule = (appName: string, ascii = newAscii, inline = newInline) => {
    const name = appName.trim();
    if (!name) return;

    if (appOptions.some((a) => a.app_name.toLowerCase() === name.toLowerCase())) {
      showNotice(`应用 “${name}” 规则已存在于列表中`);
      return;
    }

    onChange([
      { app_name: name, ascii_mode: ascii, inline_preedit: inline },
      ...appOptions,
    ]);
    setNewAppName('');
    showNotice(`已成功添加「${name}」专属规则！点击右上角或下方【保存并部署】立即生效`);
  };

  const handleDelete = (name: string) => {
    onChange(appOptions.filter((a) => a.app_name !== name));
    showNotice(`已移除应用「${name}」专属规则`);
  };

  const handleToggle = (name: string, field: 'ascii_mode' | 'inline_preedit') => {
    onChange(
      appOptions.map((a) => (a.app_name === name ? { ...a, [field]: !a[field] } : a))
    );
  };

  const handleBatchToggleAscii = (val: boolean) => {
    onChange(appOptions.map((a) => ({ ...a, ascii_mode: val })));
    showNotice(val ? '已将所有专属应用设为默认英文' : '已将所有专属应用设为保持中文');
  };

  const handleBatchToggleInline = (val: boolean) => {
    onChange(appOptions.map((a) => ({ ...a, inline_preedit: val })));
    showNotice(val ? '已将所有专属应用开启行内预编辑' : '已将所有专属应用设为框内预编辑');
  };

  const allAscii = appOptions.length > 0 && appOptions.every((a) => a.ascii_mode);
  const allInline = appOptions.length > 0 && appOptions.every((a) => a.inline_preedit);

  const toggleAllAscii = () => {
    handleBatchToggleAscii(!allAscii);
  };

  const toggleAllInline = () => {
    handleBatchToggleInline(!allInline);
  };

  const handleClearAll = () => {
    if (confirm('确定要清空所有应用程序的专属规则吗？清空后所有应用将遵循输入法全局设置。')) {
      onChange([]);
      showNotice('已清空全部专属应用规则');
    }
  };

  const presetsByPlatform = {
    windows: [
      { name: 'cmd.exe', label: 'CMD 命令行' },
      { name: 'powershell.exe', label: 'PowerShell' },
      { name: 'windowsterminal.exe', label: 'Windows Terminal' },
      { name: 'code.exe', label: 'VS Code' },
      { name: 'chrome.exe', label: 'Chrome' },
      { name: 'firefox.exe', label: 'Firefox' },
      { name: 'nvim-qt.exe', label: 'Neovim' },
    ],
    linux: [
      { name: 'mate-terminal', label: 'MATE 终端' },
      { name: 'gnome-terminal', label: 'GNOME 终端' },
      { name: 'kitty', label: 'Kitty' },
      { name: 'alacritty', label: 'Alacritty' },
      { name: 'konsole', label: 'Konsole' },
      { name: 'code', label: 'VS Code' },
      { name: 'firefox-esr', label: 'Firefox ESR' },
      { name: 'google-chrome', label: 'Chrome' },
      { name: 'caja', label: 'Caja 文件管理' },
    ],
    macos: [
      { name: 'com.apple.Terminal', label: 'Terminal 终端' },
      { name: 'com.googlecode.iterm2', label: 'iTerm2' },
      { name: 'alacritty', label: 'Alacritty' },
      { name: 'com.microsoft.VSCode', label: 'VS Code' },
      { name: 'com.google.Chrome', label: 'Chrome' },
      { name: 'org.vim.MacVim', label: 'MacVim' },
    ],
  };

  const currentPlatformKey = env?.platform || 'windows';
  const quickPresets = presetsByPlatform[currentPlatformKey] || presetsByPlatform.windows;

  // 运行中的应用
  const runningApps = systemApps.filter((a) => a.is_running);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '22px' }}>
      <div className="glass-panel" style={{ padding: '24px', borderRadius: '16px' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '10px', marginBottom: '8px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', fontSize: '18px', fontWeight: 800, color: 'var(--text-main)' }}>
            <Monitor size={22} color="#38bdf8" />
            <span>特定应用程序专属行为规则（app_options）</span>
          </div>

          {onSaveAndDeploy && (
            <button
              type="button"
              onClick={onSaveAndDeploy}
              disabled={saving}
              className="btn-primary"
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '6px',
                padding: '8px 16px',
                fontSize: '13px',
                fontWeight: 700,
                borderRadius: '8px',
                cursor: saving ? 'not-allowed' : 'pointer',
                opacity: saving ? 0.7 : 1,
              }}
            >
              {saving ? <Activity size={15} className="spin" /> : <Save size={15} />}
              <span>{saving ? '正在编译部署...' : '保存并部署生效'}</span>
            </button>
          )}
        </div>

        <p style={{ fontSize: '13px', color: 'var(--text-muted)', marginBottom: '16px' }}>
          为特定软件（如各类终端仿真器、IDE代码编辑器、网页浏览器）独立设定启动时默认处于中文或英文模式，以及是否启用行内预编辑。当前已配置 <strong style={{ color: '#38bdf8' }}>{appOptions.length}</strong> 款应用专属规则。
        </p>

        {/* 临时提示横幅 */}
        {actionNotice && (
          <div
            style={{
              padding: '10px 14px',
              borderRadius: '8px',
              backgroundColor: 'rgba(56, 189, 248, 0.15)',
              border: '1px solid rgba(56, 189, 248, 0.4)',
              color: '#38bdf8',
              fontSize: '13px',
              fontWeight: 600,
              marginBottom: '16px',
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
            }}
          >
            <Sparkles size={16} />
            <span>{actionNotice}</span>
          </div>
        )}

        {/* 顶部添加新应用规则栏 */}
        <div
          style={{
            display: 'flex',
            flexWrap: 'wrap',
            gap: '14px',
            alignItems: 'center',
            padding: '16px 20px',
            borderRadius: '12px',
            backgroundColor: 'var(--bg-tertiary)',
            border: '1px solid var(--border)',
            marginBottom: '18px',
          }}
        >
          <div style={{ flex: 1, minWidth: '260px' }}>
            <input
              type="text"
              list="system-apps-datalist"
              placeholder={
                env?.platform === 'linux'
                  ? '输入进程名或支持自动补全 (如: mate-terminal, code, firefox-esr)'
                  : env?.platform === 'macos'
                  ? '输入 Bundle ID 或程序名 (如: com.apple.Terminal, com.microsoft.VSCode)'
                  : '输入应用程序名 (如: wt.exe, powershell.exe, code.exe)'
              }
              value={newAppName}
              onChange={(e) => setNewAppName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleAdd();
              }}
              className="mono"
              style={{
                width: '100%',
                padding: '10px 14px',
                borderRadius: '8px',
                backgroundColor: 'var(--bg-elevated)',
                color: 'var(--text-main)',
                border: '1px solid var(--border)',
                fontSize: '14px',
                outline: 'none',
              }}
            />
            {/* 系统应用自动匹配推荐列表 */}
            <datalist id="system-apps-datalist">
              {systemApps.map((app) => (
                <option key={app.name} value={app.name}>
                  {app.display_name} ({app.name}){app.is_running ? ' [当前运行中]' : ''}
                </option>
              ))}
            </datalist>
          </div>

          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '14px', color: 'var(--text-main)', cursor: 'pointer', fontWeight: 600 }}>
            <input
              type="checkbox"
              checked={newAscii}
              onChange={(e) => setNewAscii(e.target.checked)}
              style={{ accentColor: '#38bdf8', width: '18px', height: '18px' }}
            />
            默认英文 (ascii_mode)
          </label>

          <label style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '14px', color: 'var(--text-main)', cursor: 'pointer', fontWeight: 600 }}>
            <input
              type="checkbox"
              checked={newInline}
              onChange={(e) => setNewInline(e.target.checked)}
              style={{ accentColor: '#38bdf8', width: '18px', height: '18px' }}
            />
            行内预编辑 (inline_preedit)
          </label>

          <button
            type="button"
            onClick={handleAdd}
            className="btn-primary"
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              padding: '10px 20px',
              borderRadius: '8px',
              fontSize: '14px',
              cursor: 'pointer',
              fontWeight: 700,
            }}
          >
            <Plus size={16} /> 添加应用规则
          </button>
        </div>

        {/* 智能检测：当前桌面正在运行中的应用（点击直接一键添加规则） */}
        {runningApps.length > 0 && (
          <div
            style={{
              padding: '12px 16px',
              borderRadius: '10px',
              backgroundColor: 'rgba(52, 211, 153, 0.08)',
              border: '1px solid rgba(52, 211, 153, 0.25)',
              marginBottom: '16px',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '8px' }}>
              <Activity size={15} color="#34d399" />
              <span style={{ fontSize: '12px', fontWeight: 700, color: '#34d399' }}>
                {scanningApps ? '正在探测系统运行应用...' : '探测到当前正在运行的桌面应用（点击即可一键加入规则）：'}
              </span>
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
              {runningApps.map((app) => {
                const exists = appOptions.some((a) => a.app_name.toLowerCase() === app.name.toLowerCase());
                return (
                  <button
                    key={app.name}
                    type="button"
                    onClick={() => {
                      if (!exists) {
                        handleAddRule(app.name, true, false);
                      } else {
                        showNotice(`「${app.display_name}」已经在下方规则列表中`);
                      }
                    }}
                    style={{
                      display: 'inline-flex',
                      alignItems: 'center',
                      gap: '6px',
                      fontSize: '12px',
                      fontWeight: 600,
                      padding: '4px 10px',
                      borderRadius: '6px',
                      backgroundColor: exists ? 'rgba(56, 189, 248, 0.15)' : 'var(--bg-elevated)',
                      border: exists ? '1px solid #38bdf8' : '1px solid var(--border)',
                      color: exists ? '#38bdf8' : 'var(--text-main)',
                      cursor: 'pointer',
                      transition: 'all 0.15s ease',
                    }}
                    title={exists ? '已经在配置规则列表中' : `点击直接为 ${app.display_name} (${app.name}) 添加默认英文规则`}
                  >
                    <span style={{ width: '7px', height: '7px', borderRadius: '50%', backgroundColor: '#34d399' }} />
                    <span>{app.display_name}</span>
                    <span className="mono" style={{ fontSize: '11px', color: 'var(--text-dim)' }}>({app.name})</span>
                    <span style={{ fontSize: '11px', color: exists ? '#38bdf8' : '#34d399', fontWeight: 700 }}>
                      {exists ? '✓ 已添加' : '+ 添加'}
                    </span>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {/* 快速推荐常用应用（点击填入并直接一键添加） */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap', marginBottom: '18px' }}>
          <span style={{ fontSize: '12px', color: 'var(--text-muted)', fontWeight: 600 }}>
            {env?.platform === 'linux'
              ? '常用 Linux 软件预设:'
              : env?.platform === 'macos'
              ? '常用 macOS 软件预设:'
              : '常用 Windows 软件预设:'}
          </span>
          {quickPresets.map((p) => {
            const exists = appOptions.some((a) => a.app_name.toLowerCase() === p.name.toLowerCase());
            return (
              <button
                key={p.name}
                type="button"
                onClick={() => {
                  if (!exists) {
                    handleAddRule(p.name, true, false);
                  } else {
                    setNewAppName(p.name);
                    showNotice(`应用「${p.label}」已添加，正在选中该名称`);
                  }
                }}
                style={{
                  fontSize: '12px',
                  fontWeight: 600,
                  padding: '4px 10px',
                  borderRadius: '6px',
                  backgroundColor: exists ? 'rgba(56, 189, 248, 0.1)' : 'var(--bg-tertiary)',
                  border: exists ? '1px solid rgba(56, 189, 248, 0.4)' : '1px solid var(--border)',
                  color: exists ? '#38bdf8' : 'var(--text-muted)',
                  cursor: 'pointer',
                  display: 'inline-flex',
                  alignItems: 'center',
                  gap: '4px',
                }}
                title={exists ? '已在规则列表中' : `点击直接添加 ${p.label} (${p.name})`}
              >
                <span>{p.label}</span>
                <span className="mono" style={{ fontSize: '11px', color: 'var(--text-dim)' }}>({p.name})</span>
                {exists ? <span style={{ fontSize: '10px', color: '#38bdf8' }}>✓</span> : <span style={{ fontSize: '11px', color: '#38bdf8' }}>+</span>}
              </button>
            );
          })}
        </div>

        {/* Linux 专属小贴士折叠指南 */}
        {env?.platform === 'linux' && (
          <div
            style={{
              padding: '12px 16px',
              borderRadius: '10px',
              backgroundColor: 'var(--bg-tertiary)',
              border: '1px solid var(--border)',
              marginBottom: '18px',
            }}
          >
            <div
              onClick={() => setShowGuide(!showGuide)}
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
                cursor: 'pointer',
                userSelect: 'none',
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '13px', fontWeight: 700, color: '#38bdf8' }}>
                <HelpCircle size={16} />
                <span>💡 实用指南：在 Linux 中如何精准获取任意窗口的真实进程名？</span>
              </div>
              {showGuide ? <ChevronUp size={16} color="var(--text-dim)" /> : <ChevronDown size={16} color="var(--text-dim)" />}
            </div>

            {showGuide && (
              <div style={{ marginTop: '10px', fontSize: '12px', color: 'var(--text-muted)', lineHeight: '1.7', borderTop: '1px solid var(--border-light)', paddingTop: '10px' }}>
                <p style={{ margin: '4px 0' }}>
                  <strong>方法一（100% 精准对应 Rime 识别）：</strong>
                  在终端运行 <code style={{ color: '#38bdf8' }}>xprop WM_CLASS</code>，鼠标光标会变成十字星，点击任意软件窗口，终端输出的第一个引号内容即为 Rime 的匹配程序名（如 <code style={{ color: '#34d399' }}>"mate-terminal"</code>、<code style={{ color: '#34d399' }}>"firefox-esr"</code>、<code style={{ color: '#34d399' }}>"code"</code>）。
                </p>
                <p style={{ margin: '4px 0' }}>
                  <strong>方法二（终端快速检索）：</strong>
                  运行 <code style={{ color: '#38bdf8' }}>ps -u $USER -o comm | sort -u</code> 查看当前用户启动的所有进程名，或用 <code style={{ color: '#38bdf8' }}>pgrep -l -a 软件名</code> 检索。
                </p>
                <p style={{ margin: '4px 0' }}>
                  <strong>方法三（直接使用 RimeTune 智能探测）：</strong>
                  直接查看上方「探测到当前正在运行的桌面应用」，只要软件开着，点击即自动填入！
                </p>
              </div>
            )}
          </div>
        )}

        {/* 批量操作工具栏 */}
        {appOptions.length > 0 && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              flexWrap: 'wrap',
              gap: '12px',
              padding: '10px 16px',
              borderRadius: '8px',
              backgroundColor: 'var(--bg-tertiary)',
              border: '1px solid var(--border)',
              marginBottom: '14px',
              fontSize: '13px',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '14px', flexWrap: 'wrap' }}>
              <span style={{ color: 'var(--text-muted)', fontWeight: 600 }}>
                已配置 <strong style={{ color: '#38bdf8' }}>{appOptions.length}</strong> 款应用
              </span>
              <span style={{ color: 'var(--border)' }}>|</span>

              {/* 英文模式一键全选/全取消 */}
              <button
                type="button"
                onClick={toggleAllAscii}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                  padding: '5px 12px',
                  borderRadius: '6px',
                  fontSize: '12px',
                  fontWeight: 600,
                  backgroundColor: allAscii ? 'rgba(56, 189, 248, 0.2)' : 'var(--bg-elevated)',
                  color: allAscii ? '#38bdf8' : 'var(--text-muted)',
                  border: allAscii ? '1px solid #38bdf8' : '1px solid var(--border)',
                  cursor: 'pointer',
                  transition: 'all 0.15s ease',
                }}
              >
                <CheckSquare size={13} color={allAscii ? '#38bdf8' : '#94a3b8'} />
                <span>英文模式 (ascii): {allAscii ? '全选 (开)' : '全选 (关)'}</span>
              </button>

              {/* 行内预编辑一键全选/全取消 */}
              <button
                type="button"
                onClick={toggleAllInline}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                  padding: '5px 12px',
                  borderRadius: '6px',
                  fontSize: '12px',
                  fontWeight: 600,
                  backgroundColor: allInline ? 'rgba(56, 189, 248, 0.2)' : 'var(--bg-elevated)',
                  color: allInline ? '#38bdf8' : 'var(--text-muted)',
                  border: allInline ? '1px solid #38bdf8' : '1px solid var(--border)',
                  cursor: 'pointer',
                  transition: 'all 0.15s ease',
                }}
              >
                <CheckSquare size={13} color={allInline ? '#38bdf8' : '#94a3b8'} />
                <span>行内预编辑: {allInline ? '全选 (开)' : '全选 (关)'}</span>
              </button>
            </div>

            <button
              type="button"
              onClick={handleClearAll}
              style={{
                padding: '4px 12px',
                borderRadius: '6px',
                fontSize: '12px',
                fontWeight: 600,
                backgroundColor: 'rgba(239, 68, 68, 0.12)',
                color: '#f87171',
                border: '1px solid rgba(239, 68, 68, 0.3)',
                cursor: 'pointer',
              }}
            >
              清空全部规则
            </button>
          </div>
        )}

        {/* 规则数据表格 */}
        <div
          style={{
            maxHeight: '480px',
            overflowY: 'auto',
            border: '1px solid var(--border)',
            borderRadius: '12px',
            backgroundColor: 'var(--bg-primary)',
          }}
        >
          <table style={{ width: '100%', borderCollapse: 'collapse', textAlign: 'left', fontSize: '14px' }}>
            <thead>
              <tr style={{ backgroundColor: 'var(--bg-tertiary)', borderBottom: '1px solid var(--border)', color: 'var(--text-muted)' }}>
                <th style={{ padding: '14px 18px', width: '38%', fontWeight: 700 }}>应用程序 / 进程标识符</th>
                <th style={{ padding: '14px 18px', width: '26%', fontWeight: 700, textAlign: 'center' }}>
                  <div style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
                    <span>默认英文 (ascii_mode)</span>
                  </div>
                </th>
                <th style={{ padding: '14px 18px', width: '26%', fontWeight: 700, textAlign: 'center' }}>
                  <div style={{ display: 'inline-flex', alignItems: 'center', gap: '8px' }}>
                    <span>行内预编辑 (inline_preedit)</span>
                  </div>
                </th>
                <th style={{ padding: '14px 18px', width: '10%', fontWeight: 700, textAlign: 'center' }}>操作</th>
              </tr>
            </thead>
            <tbody>
              {appOptions.map((item) => (
                <tr
                  key={item.app_name}
                  style={{ borderBottom: '1px solid var(--border-light)', transition: 'background-color 0.15s ease' }}
                >
                  {/* 应用名称 */}
                  <td style={{ padding: '12px 18px' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <Shield size={16} color="#38bdf8" />
                      <span className="mono" style={{ color: 'var(--text-main)', fontWeight: 600 }}>
                        {item.app_name}
                      </span>
                    </div>
                  </td>

                  {/* 默认英文勾选 */}
                  <td style={{ padding: '12px 18px', textAlign: 'center' }}>
                    <label style={{ display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={item.ascii_mode}
                        onChange={() => handleToggle(item.app_name, 'ascii_mode')}
                        style={{ accentColor: '#38bdf8', width: '18px', height: '18px', cursor: 'pointer' }}
                      />
                      <span style={{ fontSize: '13px', color: item.ascii_mode ? '#38bdf8' : 'var(--text-dim)' }}>
                        {item.ascii_mode ? '默认英文' : '保持中文'}
                      </span>
                    </label>
                  </td>

                  {/* 行内预编辑勾选 */}
                  <td style={{ padding: '12px 18px', textAlign: 'center' }}>
                    <label style={{ display: 'inline-flex', alignItems: 'center', gap: '6px', cursor: 'pointer' }}>
                      <input
                        type="checkbox"
                        checked={item.inline_preedit}
                        onChange={() => handleToggle(item.app_name, 'inline_preedit')}
                        style={{ accentColor: '#38bdf8', width: '18px', height: '18px', cursor: 'pointer' }}
                      />
                      <span style={{ fontSize: '13px', color: item.inline_preedit ? '#34d399' : 'var(--text-dim)' }}>
                        {item.inline_preedit ? '开启行内' : '框内预编辑'}
                      </span>
                    </label>
                  </td>

                  {/* 删除 */}
                  <td style={{ padding: '12px 18px', textAlign: 'center' }}>
                    <button
                      onClick={() => handleDelete(item.app_name)}
                      style={{
                        background: 'transparent',
                        border: 'none',
                        color: '#f43f5e',
                        cursor: 'pointer',
                        padding: '6px',
                      }}
                      title="删除规则"
                    >
                      <Trash2 size={18} />
                    </button>
                  </td>
                </tr>
              ))}
              {appOptions.length === 0 && (
                <tr>
                  <td colSpan={4} style={{ padding: '50px', textAlign: 'center', color: '#94a3b8' }}>
                    暂无特定应用专属规则，可点击上方「探测运行中应用」或「预设软件」快速添加
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
