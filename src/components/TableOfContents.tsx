import { TocItem } from "../types/book";
import "./TableOfContents.css";

interface TableOfContentsProps {
  toc: TocItem[];
  onItemClick: (content: string) => void;
}

interface TocItemComponentProps {
  item: TocItem;
  level: number;
  onItemClick: (content: string) => void;
}

function TocItemComponent({ item, level, onItemClick }: TocItemComponentProps) {
  const handleClick = () => {
    onItemClick(item.content);
  };

  return (
    <li className="toc-item" style={{ paddingLeft: `${level * 20}px` }}>
      <button className="toc-button" onClick={handleClick}>
        {item.label}
      </button>
      {item.children.length > 0 && (
        <ul className="toc-list">
          {item.children.map((child, index) => (
            <TocItemComponent
              key={`${child.content}-${index}`}
              item={child}
              level={level + 1}
              onItemClick={onItemClick}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

function TableOfContents({ toc, onItemClick }: TableOfContentsProps) {
  if (toc.length === 0) {
    return (
      <div className="table-of-contents">
        <h3 className="toc-header">Contents</h3>
        <p className="toc-empty">No table of contents available</p>
      </div>
    );
  }

  return (
    <div className="table-of-contents">
      <h3 className="toc-header">Contents</h3>
      <ul className="toc-list">
        {toc.map((item, index) => (
          <TocItemComponent
            key={`${item.content}-${index}`}
            item={item}
            level={0}
            onItemClick={onItemClick}
          />
        ))}
      </ul>
    </div>
  );
}

export default TableOfContents;
