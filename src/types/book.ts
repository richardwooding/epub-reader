export interface TocItem {
  label: string;
  content: string;
  play_order: number;
  children: TocItem[];
}
