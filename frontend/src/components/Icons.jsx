function SvgIcon({ className, viewBox = '0 0 24 24', children }) {
  return (
    <svg className={className} viewBox={viewBox} aria-hidden="true">
      {children}
    </svg>
  );
}

export function LightBulbIcon({ on = false, className }) {
  return (
    <SvgIcon className={className}>
      {on && (
        <>
          <path
            d="M12 3.2 13.15 5.5 15.7 5.85 13.85 7.6 14.3 10.1 12 8.95 9.7 10.1 10.15 7.6 8.3 5.85 10.85 5.5Z"
            fill="currentColor"
            opacity="0.18"
          />
          <path
            d="M12 1.8v2.1M5.45 4.55l1.5 1.5M18.55 4.55l-1.5 1.5M2.8 11.2h2.1M19.1 11.2h2.1M6.1 17.45l1.45-.85M17.9 17.45l-1.45-.85"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.55"
            strokeLinecap="round"
            opacity="0.95"
          />
        </>
      )}
      <path
        d="M8.2 10.15a3.8 3.8 0 1 1 7.6 0c0 1.32-.55 2.34-1.68 3.54-.76.8-1.08 1.35-1.17 2.01h-1.9c-.1-.66-.41-1.2-1.17-2.01-1.13-1.2-1.68-2.22-1.68-3.54Z"
        fill="currentColor"
        opacity={on ? '0.24' : '0.08'}
      />
      <path
        d="M9 15.85h6M9.45 18.15h5.1M10.35 20.35h3.3"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.65"
        strokeLinecap="round"
      />
      <path
        d="M8.2 10.15a3.8 3.8 0 1 1 7.6 0c0 1.32-.55 2.34-1.68 3.54-.76.8-1.08 1.35-1.17 2.01h-1.9c-.1-.66-.41-1.2-1.17-2.01-1.13-1.2-1.68-2.22-1.68-3.54Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.65"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M10.5 11.05c.35-1.25 1.4-2 2.75-2.2"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinecap="round"
        opacity={on ? '0.85' : '0.45'}
      />
    </SvgIcon>
  );
}

export function NozzleTempIcon({ className }) {
  return (
    <SvgIcon className={className}>
      <path
        d="M8.1 4.2h7.8v2.55h-1.55v2.95l2.1 2.45v1.55H7.55V12.15l2.1-2.45V6.75H8.1Z"
        fill="currentColor"
        opacity="0.08"
      />
      <path
        d="M8.1 4.2h7.8v2.55h-1.55v2.95l2.1 2.45v1.55H7.55V12.15l2.1-2.45V6.75H8.1Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.55"
        strokeLinejoin="round"
      />
      <path
        d="M10.05 15.75h3.9M9.2 18.2h5.6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.55"
        strokeLinecap="round"
      />
      <path
        d="M19 7.1c.95 1 1.02 2.35.15 3.38M17.15 6.15c1.4 1.45 1.48 3.5.18 4.98"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.4"
        strokeLinecap="round"
        opacity="0.85"
      />
      <path
        d="M11.95 10.15c1.02 1.2 1.55 1.98 1.55 2.75a1.55 1.55 0 0 1-3.1 0c0-.77.53-1.55 1.55-2.75Z"
        fill="currentColor"
        opacity="0.22"
      />
    </SvgIcon>
  );
}

export function BedTempIcon({ className }) {
  return (
    <SvgIcon className={className}>
      <path
        d="M5.2 9.2h13.6v4.65H5.2Z"
        fill="currentColor"
        opacity="0.08"
      />
      <rect
        x="5.2"
        y="9.2"
        width="13.6"
        height="4.65"
        rx="1.6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.55"
      />
      <path
        d="M7.7 15.95h8.6M9.2 18.35h5.6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.55"
        strokeLinecap="round"
      />
      <path
        d="M8.2 7.2c-.65-.72-.7-1.6-.15-2.35.6-.8.58-1.58-.08-2.45M12 7.2c-.65-.72-.7-1.6-.15-2.35.6-.8.58-1.58-.08-2.45M15.8 7.2c-.65-.72-.7-1.6-.15-2.35.6-.8.58-1.58-.08-2.45"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.35"
        strokeLinecap="round"
        opacity="0.88"
      />
      <path
        d="M8.25 11.55h7.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.15"
        strokeLinecap="round"
        opacity="0.45"
      />
    </SvgIcon>
  );
}
