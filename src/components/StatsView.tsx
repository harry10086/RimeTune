import { useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ChartColumnBig, RefreshCw } from 'lucide-react';

interface DayStat {
  date: string;
  chars: number;
}

const pad = (n: number) => String(n).padStart(2, '0');
const keyOf = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const shortDate = (d: Date) => `${d.getMonth() + 1}/${d.getDate()}`;

function lastDays(n: number): Date[] {
  const out: Date[] = [];
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  for (let i = n - 1; i >= 0; i--) {
    const d = new Date(today);
    d.setDate(today.getDate() - i);
    out.push(d);
  }
  return out;
}

function fmtNum(n: number): string {
  return n.toLocaleString('zh-CN');
}

export function StatsView() {
  const [days, setDays] = useState<DayStat[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await invoke<DayStat[]>('read_typing_stats');
      setDays(res ?? []);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    load();
  }, []);

  const map = useMemo(() => {
    const m = new Map<string, number>();
    days.forEach((d) => m.set(d.date, (m.get(d.date) ?? 0) + d.chars));
    return m;
  }, [days]);

  const week = lastDays(7);
  const month = lastDays(30);
  const todayKey = keyOf(new Date());

  const sum = (list: Date[]) => list.reduce((s, d) => s + (map.get(keyOf(d)) ?? 0), 0);
  const total = days.reduce((s, d) => s + d.chars, 0);
  const today = map.get(todayKey) ?? 0;
  const last7 = sum(week);
  const last30 = sum(month);
  const monthMax = Math.max(1, ...month.map((d) => map.get(keyOf(d)) ?? 0));
  const activeDays = month.filter((d) => (map.get(keyOf(d)) ?? 0) > 0).length;
  const avgActive = activeDays > 0 ? Math.round(last30 / activeDays) : 0;
  const bestDay = days.reduce<DayStat | null>((best, d) => (!best || d.chars > best.chars ? d : best), null);

  // 热力图：最近 12 周，按周一到周日排列
  const heat = useMemo(() => {
    const todayDate = new Date();
    todayDate.setHours(0, 0, 0, 0);
    const dow = (todayDate.getDay() + 6) % 7; // 0 = 周一
    const start = new Date(todayDate);
    start.setDate(todayDate.getDate() - dow - 11 * 7);
    const weeks: Date[][] = [];
    for (let w = 0; w < 12; w++) {
      const col: Date[] = [];
      for (let d = 0; d < 7; d++) {
        const dt = new Date(start);
        dt.setDate(start.getDate() + w * 7 + d);
        col.push(dt);
      }
      weeks.push(col);
    }
    return weeks;
  }, [days]);

  const heatMax = Math.max(1, ...days.map((d) => d.chars));
  const levelOf = (v: number) => {
    if (v <= 0) return 0;
    const r = v / heatMax;
    if (r < 0.25) return 1;
    if (r < 0.5) return 2;
    if (r < 0.75) return 3;
    return 4;
  };
  const levelColor = (lv: number) => {
    switch (lv) {
      case 1:
        return 'rgba(56, 189, 248, 0.25)';
      case 2:
        return 'rgba(56, 189, 248, 0.45)';
      case 3:
        return 'rgba(56, 189, 248, 0.7)';
      case 4:
        return '#38bdf8';
      default:
        return 'rgba(255, 255, 255, 0.05)';
    }
  };

  const card: React.CSSProperties = {
    backgroundColor: 'var(--bg-card)',
    border: '1px solid var(--border)',
    borderRadius: '14px',
    padding: '18px 20px',
  };

  const kpiLabel: React.CSSProperties = {
    fontSize: '12px',
    color: 'var(--text-dim)',
    letterSpacing: '0.3px',
    marginBottom: '8px',
  };

  const kpiValue: React.CSSProperties = {
    fontSize: '26px',
    fontWeight: 800,
    color: 'var(--text-main)',
    lineHeight: 1.1,
  };

  const hasData = days.length > 0 && total > 0;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '18px', maxWidth: '1200px' }}>
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: '16px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <div
            style={{
              width: '38px',
              height: '38px',
              borderRadius: '10px',
              backgroundColor: 'var(--accent-glow)',
              border: '1px solid var(--border-accent)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'var(--accent)',
            }}
          >
            <ChartColumnBig size={20} />
          </div>
          <div>
            <div style={{ fontSize: '18px', fontWeight: 800, color: 'var(--text-main)' }}>打字统计</div>
            <div style={{ fontSize: '12px', color: 'var(--text-dim)', marginTop: '2px' }}>
              只统计每天上屏的字数，不记录任何输入内容 · 数据存于 <code>%APPDATA%\Rime\typing_stats.jsonl</code>
            </div>
          </div>
        </div>
        <button
          type="button"
          onClick={load}
          disabled={loading}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            gap: '8px',
            padding: '9px 16px',
            borderRadius: '10px',
            border: '1px solid var(--border-accent)',
            backgroundColor: 'var(--accent-glow)',
            color: 'var(--accent)',
            fontSize: '13px',
            fontWeight: 700,
            cursor: loading ? 'default' : 'pointer',
            opacity: loading ? 0.6 : 1,
          }}
        >
          <RefreshCw size={15} className={loading ? 'spin' : ''} />
          {loading ? '读取中...' : '刷新'}
        </button>
      </div>

      {error && (
        <div
          style={{
            ...card,
            borderColor: 'rgba(244, 63, 94, 0.4)',
            color: 'var(--accent-rose)',
            fontSize: '13px',
          }}
        >
          读取统计失败：{error}
        </div>
      )}

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, minmax(0, 1fr))', gap: '14px' }}>
        <div style={card}>
          <div style={kpiLabel}>今日</div>
          <div style={kpiValue}>
            {fmtNum(today)} <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-dim)' }}>字</span>
          </div>
        </div>
        <div style={card}>
          <div style={kpiLabel}>近 7 天</div>
          <div style={kpiValue}>
            {fmtNum(last7)} <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-dim)' }}>字</span>
          </div>
        </div>
        <div style={card}>
          <div style={kpiLabel}>近 30 天</div>
          <div style={kpiValue}>
            {fmtNum(last30)} <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-dim)' }}>字</span>
          </div>
        </div>
        <div style={card}>
          <div style={kpiLabel}>累计</div>
          <div style={kpiValue}>
            {fmtNum(total)} <span style={{ fontSize: '13px', fontWeight: 500, color: 'var(--text-dim)' }}>字</span>
          </div>
        </div>
      </div>

      <div style={{ ...card, padding: '20px 22px' }}>
        <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', marginBottom: '16px' }}>
          <div style={{ fontSize: '14px', fontWeight: 700, color: 'var(--text-primary)' }}>近 30 天</div>
          <div style={{ fontSize: '12px', color: 'var(--text-dim)' }}>
            活跃 {activeDays} 天 · 活跃日日均 {fmtNum(avgActive)} 字
            {bestDay && bestDay.chars > 0 ? ` · 最高单日 ${fmtNum(bestDay.chars)} 字（${bestDay.date}）` : ''}
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'flex-end', gap: '6px', height: '170px' }}>
          {month.map((d) => {
            const v = map.get(keyOf(d)) ?? 0;
            const h = v > 0 ? Math.max(3, Math.round((v / monthMax) * 150)) : 2;
            return (
              <div
                key={keyOf(d)}
                title={`${keyOf(d)}：${fmtNum(v)} 字`}
                style={{ flex: 1, display: 'flex', flexDirection: 'column', justifyContent: 'flex-end', height: '100%' }}
              >
                <div
                  style={{
                    height: `${h}px`,
                    borderRadius: '4px 4px 2px 2px',
                    backgroundColor: v > 0 ? 'var(--accent)' : 'rgba(255,255,255,0.06)',
                    transition: 'height 0.2s ease',
                  }}
                />
                <div
                  style={{
                    fontSize: '9px',
                    color: 'var(--text-dim)',
                    textAlign: 'center',
                    marginTop: '6px',
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                  }}
                >
                  {d.getDate() === 1 || d.getDate() % 5 === 0 ? shortDate(d) : ''}
                </div>
              </div>
            );
          })}
        </div>
      </div>

      <div style={{ ...card, padding: '20px 22px' }}>
        <div style={{ fontSize: '14px', fontWeight: 700, color: 'var(--text-primary)', marginBottom: '16px' }}>
          最近 12 周
        </div>
        <div style={{ display: 'flex', gap: '10px', alignItems: 'flex-start' }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', paddingTop: '0px' }}>
            {['一', '', '三', '', '五', '', '日'].map((t, i) => (
              <div key={i} style={{ height: '13px', fontSize: '9px', color: 'var(--text-dim)', lineHeight: '13px', width: '10px' }}>
                {t}
              </div>
            ))}
          </div>
          <div style={{ display: 'flex', gap: '4px' }}>
            {heat.map((col, wi) => (
              <div key={wi} style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                {col.map((d) => {
                  const k = keyOf(d);
                  const v = map.get(k) ?? 0;
                  const future = d > new Date();
                  return (
                    <div
                      key={k}
                      title={`${k}：${fmtNum(v)} 字`}
                      style={{
                        width: '13px',
                        height: '13px',
                        borderRadius: '3px',
                        backgroundColor: future ? 'transparent' : levelColor(levelOf(v)),
                        border: future ? '1px dashed rgba(255,255,255,0.06)' : 'none',
                      }}
                    />
                  );
                })}
              </div>
            ))}
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '6px', marginTop: '14px', fontSize: '11px', color: 'var(--text-dim)' }}>
          <span>少</span>
          {[0, 1, 2, 3, 4].map((lv) => (
            <div key={lv} style={{ width: '11px', height: '11px', borderRadius: '3px', backgroundColor: levelColor(lv) }} />
          ))}
          <span>多</span>
        </div>
      </div>

      {!hasData && !loading && (
        <div style={{ ...card, borderStyle: 'dashed', color: 'var(--text-muted)', fontSize: '13px', lineHeight: 1.9 }}>
          <div style={{ fontWeight: 700, color: 'var(--text-primary)', marginBottom: '6px' }}>还没有统计数据</div>
          统计模块需要先在输入法里生效：确认 <code>%APPDATA%\Rime\lua\input_stats.lua</code> 存在、方案里挂了{' '}
          <code>lua_filter@*input_stats</code>，然后部署一次。之后正常打字，字数会自动累计，再回来点「刷新」。
        </div>
      )}
    </div>
  );
}
