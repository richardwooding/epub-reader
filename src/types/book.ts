export interface TocItem {
  label: string;
  content: string;
  play_order: number;
  children: TocItem[];
}

export interface ReadingPosition {
  bookKey: string;
  contentPath: string;    // Without epub:// prefix
  page: number;           // 0-indexed
  timestamp: number;
}
