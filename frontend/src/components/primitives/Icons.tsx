/** Inline glyphs. All take currentColor so material classes set the ink. */

export const ReturnIcon = () => (
  <svg width="12" height="10" viewBox="0 0 12 10">
    <rect x="0" y="0" width="2" height="10" fill="currentColor" />
    <path d="M11 0 L3 5 L11 10Z" fill="currentColor" />
  </svg>
);

export const RewindIcon = () => (
  <svg width="14" height="10" viewBox="0 0 14 10">
    <path d="M7 0 L0 5 L7 10Z M14 0 L7 5 L14 10Z" fill="currentColor" />
  </svg>
);

export const ForwardIcon = () => (
  <svg width="14" height="10" viewBox="0 0 14 10">
    <path d="M0 0 L7 5 L0 10Z M7 0 L14 5 L7 10Z" fill="currentColor" />
  </svg>
);

export const PlayIcon = () => (
  <svg width="11" height="12" viewBox="0 0 11 12">
    <path d="M0 0 L11 6 L0 12Z" fill="currentColor" />
  </svg>
);

export const PlaySmallIcon = () => (
  <svg width="8" height="9" viewBox="0 0 8 9">
    <path d="M0 0 L8 4.5 L0 9Z" fill="currentColor" />
  </svg>
);

export const StopIcon = () => (
  <svg width="10" height="10" viewBox="0 0 10 10">
    <rect width="10" height="10" rx="1" fill="currentColor" />
  </svg>
);

export const RecordIcon = () => (
  <svg width="12" height="12" viewBox="0 0 12 12">
    <circle cx="6" cy="6" r="5" fill="currentColor" />
  </svg>
);

export const RecordSmallIcon = () => (
  <svg width="7" height="7" viewBox="0 0 7 7">
    <circle cx="3.5" cy="3.5" r="3.5" fill="currentColor" />
  </svg>
);

export const CycleIcon = () => (
  <svg
    width="14"
    height="12"
    viewBox="0 0 14 12"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.6"
  >
    <path d="M2 6 a5 5 0 0 1 9 -3 M12 6 a5 5 0 0 1 -9 3" />
    <path d="M11 0 v3 h-3 M3 12 v-3 h3" />
  </svg>
);

export const SearchIcon = () => (
  <svg
    width="11"
    height="11"
    viewBox="0 0 11 11"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
  >
    <circle cx="4.5" cy="4.5" r="3.5" />
    <path d="M7 7 L10 10" />
  </svg>
);

export const PointerIcon = () => (
  <svg width="9" height="12" viewBox="0 0 9 12">
    <path d="M0 0 L9 8 L5 8 L7 12 L5 12 L3 8.5 L0 11Z" fill="currentColor" />
  </svg>
);

export const PencilIcon = () => (
  <svg
    width="11"
    height="11"
    viewBox="0 0 11 11"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
  >
    <path d="M1 10 L2 7 L8 1 L10 3 L4 9Z" />
  </svg>
);

export const ScissorsIcon = () => (
  <svg
    width="11"
    height="11"
    viewBox="0 0 11 11"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
  >
    <circle cx="3" cy="8" r="2" />
    <circle cx="8" cy="8" r="2" />
    <path d="M4.5 6.5 L9 0 M6.5 6.5 L2 0" />
  </svg>
);

export const GridIcon = () => (
  <svg
    width="11"
    height="11"
    viewBox="0 0 11 11"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
  >
    <path d="M1 3 H10 M1 8 H10 M3 1 V10 M8 1 V10" />
  </svg>
);

export const ChevronRightIcon = () => (
  <svg
    width="10"
    height="10"
    viewBox="0 0 10 10"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.5"
  >
    <path d="M3 1 L7 5 L3 9" />
  </svg>
);

export const SendIcon = () => (
  <svg
    width="11"
    height="11"
    viewBox="0 0 11 11"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.8"
  >
    <path d="M5.5 10 V1 M1.5 5 L5.5 1 L9.5 5" />
  </svg>
);

export const StarIcon = ({ filled = false }: { filled?: boolean }) => (
  <svg
    width="11"
    height="11"
    viewBox="0 0 12 12"
    fill={filled ? "currentColor" : "none"}
    stroke="currentColor"
    strokeWidth="1.1"
    strokeLinejoin="round"
  >
    <path d="M6 1.2 7.5 4.3 10.9 4.8 8.4 7.1 9 10.5 6 8.9 3 10.5 3.6 7.1 1.1 4.8 4.5 4.3Z" />
  </svg>
);
