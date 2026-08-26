/**
 * 语言切换徽章:圆角方形拟物风格(参考锤子科技语言切换图标)
 * 统一 3:2 圆角矩形 + 细描边 + 顶部高光,尺寸由 className 控制
 */

function BadgeShell({ children }: { children: React.ReactNode }) {
  return (
    <svg
      viewBox="0 0 30 20"
      className="w-[18px] h-[12px] shrink-0"
      aria-hidden="true"
    >
      <defs>
        <clipPath id="lang-badge-clip">
          <rect x="0.5" y="0.5" width="29" height="19" rx="5" />
        </clipPath>
        <linearGradient id="lang-badge-gloss" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="#ffffff" stopOpacity="0.35" />
          <stop offset="45%" stopColor="#ffffff" stopOpacity="0.06" />
          <stop offset="100%" stopColor="#000000" stopOpacity="0.06" />
        </linearGradient>
      </defs>
      <g clipPath="url(#lang-badge-clip)">{children}</g>
      {/* 顶部高光 + 边框,营造 app 图标般的立体圆角质感 */}
      <rect x="0.5" y="0.5" width="29" height="19" rx="5" fill="url(#lang-badge-gloss)" />
      <rect
        x="0.5"
        y="0.5"
        width="29"
        height="19"
        rx="5"
        fill="none"
        stroke="#000000"
        strokeOpacity="0.12"
        strokeWidth="1"
      />
    </svg>
  );
}

/** 五角星路径(外接圆半径 r,圆心平移到 cx,cy,一个角朝上) */
function starPath(cx: number, cy: number, r: number): string {
  const pts: string[] = [];
  for (let i = 0; i < 5; i++) {
    // 外点(从正上方开始)与内点交替
    const outerA = -Math.PI / 2 + (i * 2 * Math.PI) / 5;
    const innerA = outerA + Math.PI / 5;
    const ir = (r * 38) / 95; // 内外半径比 ≈0.4(标准五角星)
    pts.push(
      `${(cx + r * Math.cos(outerA)).toFixed(2)},${(cy + r * Math.sin(outerA)).toFixed(2)}`,
      `${(cx + ir * Math.cos(innerA)).toFixed(2)},${(cy + ir * Math.sin(innerA)).toFixed(2)}`,
    );
  }
  return `M${pts.join("L")}Z`;
}

export function FlagCN() {
  return (
    <BadgeShell>
      <rect width="30" height="20" fill="#EE1C25" />
      <path d={starPath(6, 5.5, 3.2)} fill="#FF0" />
      <path d={starPath(11, 2.4, 1.1)} fill="#FF0" />
      <path d={starPath(13, 4.6, 1.1)} fill="#FF0" />
      <path d={starPath(13, 7.6, 1.1)} fill="#FF0" />
      <path d={starPath(11, 9.6, 1.1)} fill="#FF0" />
    </BadgeShell>
  );
}

export function FlagGB() {
  return (
    <BadgeShell>
      <rect width="30" height="20" fill="#012169" />
      {/* 斜十字:白底 + 红线(Saint Patrick 简化) */}
      <path d="M0,0 L30,20 M30,0 L0,20" stroke="#ffffff" strokeWidth="4" />
      <path d="M0,0 L30,20 M30,0 L0,20" stroke="#C8102E" strokeWidth="1.6" />
      {/* 正十字:白底 + 红线 */}
      <path d="M15,0 V20 M0,10 H30" stroke="#ffffff" strokeWidth="6.5" />
      <path d="M15,0 V20 M0,10 H30" stroke="#C8102E" strokeWidth="3.8" />
    </BadgeShell>
  );
}

export function GlobeAuto() {
  return (
    <BadgeShell>
      <defs>
        <linearGradient id="lang-globe-bg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0%" stopColor="#38bdf8" />
          <stop offset="100%" stopColor="#6366f1" />
        </linearGradient>
      </defs>
      <rect width="30" height="20" fill="url(#lang-globe-bg)" />
      {/* 经纬线地球 */}
      <g stroke="#ffffff" strokeOpacity="0.9" fill="none" strokeWidth="0.9">
        <circle cx="15" cy="10" r="6.4" />
        <ellipse cx="15" cy="10" rx="2.9" ry="6.4" />
        <line x1="8.6" y1="10" x2="21.4" y2="10" />
        <path d="M9.4 6.6 H20.6 M9.4 13.4 H20.6" />
      </g>
    </BadgeShell>
  );
}
