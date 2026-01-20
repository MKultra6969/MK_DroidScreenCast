import { useEffect, useMemo, useRef, useState, type DragEvent } from 'react';
import {
  ArrowUp,
  ArrowRight,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Download,
  FileText,
  Folder,
  FolderOpen,
  FolderPlus,
  Monitor,
  Pencil,
  RefreshCw,
  Trash2,
  Upload,
  X
} from 'lucide-react';
import { API_BASE } from '../../lib/api';
import { delayStyle } from '../../lib/style';
import { cn } from '../../utils';
import type { FileEntry, Screenshot } from '../../types/app';

type FilesPageProps = {
  t: (key: string) => string;
  activeSection: string;
  isSectionCollapsed: (sectionId: string) => boolean;
  toggleSection: (sectionId: string) => void;
  screenshots: Screenshot[];
  screenshotsLoading: boolean;
  takingScreenshot: boolean;
  loadScreenshots: () => void | Promise<void>;
  takeScreenshot: (caption?: string) => Promise<boolean> | boolean;
  downloadScreenshot: (id: string, filename: string) => void | Promise<void>;
  deleteScreenshot: (id: string) => Promise<boolean> | boolean;
  updateScreenshotCaption: (id: string, caption: string) => Promise<boolean> | boolean;
  screenshotsPage: number;
  screenshotsPageSize: number;
  screenshotsTotal: number;
  screenshotsTotalPages: number;
  onScreenshotsPageChange: (page: number) => void;
  onScreenshotsPageSizeChange: (size: number) => void;
  currentPath: string;
  files: FileEntry[];
  filesLoading: boolean;
  filesBusy: boolean;
  filesError: string;
  refreshFiles: () => void | Promise<void>;
  navigateToDir: (path: string) => void;
  downloadFile: (entry: FileEntry) => void | Promise<void>;
  deleteFile: (entry: FileEntry) => void | Promise<void>;
  createDirectory: (path: string) => void | Promise<void>;
  moveEntry: (source: string, destination: string) => void | Promise<void>;
  readFile: (path: string) => Promise<{ content: string; truncated: boolean; isBinary: boolean }>;
  writeFile: (path: string, content: string) => Promise<boolean> | boolean;
  uploadFiles: (files: FileList, destination: string) => void | Promise<void>;
  filesPage: number;
  filesPageSize: number;
  filesTotal: number;
  filesTotalPages: number;
  onFilesPageChange: (page: number) => void;
  onFilesPageSizeChange: (size: number) => void;
};

const INTERNAL_DRAG_TYPE = 'application/x-mkdsc-path';

const formatBytes = (value: number) => {
  if (!value) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  let size = value;
  let unitIndex = 0;
  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }
  return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
};

const joinPath = (base: string, name: string) => {
  if (base === '/' || !base) {
    return `/${name}`;
  }
  return `${base.replace(/\/$/, '')}/${name}`;
};

const getParentPath = (path: string) => {
  if (!path || path === '/') return '/';
  const parts = path.split('/').filter(Boolean);
  if (parts.length <= 1) return '/';
  return `/${parts.slice(0, -1).join('/')}`;
};

const basename = (path: string) => {
  const parts = path.split('/').filter(Boolean);
  return parts[parts.length - 1] || path;
};

const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max);

type PaginationLabels = {
  prev: string;
  next: string;
  page: string;
  of: string;
  perPage: string;
  showing: string;
  empty: string;
};

type PaginationProps = {
  page: number;
  pageSize: number;
  totalPages: number;
  totalCount: number;
  sizes: number[];
  disabled?: boolean;
  labels: PaginationLabels;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: number) => void;
};

const PaginationBar = ({
  page,
  pageSize,
  totalPages,
  totalCount,
  sizes,
  disabled,
  labels,
  onPageChange,
  onPageSizeChange
}: PaginationProps) => {
  const safeTotalPages = Math.max(totalPages, 1);
  const safePage = clamp(page, 1, safeTotalPages);
  const startIndex = totalCount === 0 ? 0 : (safePage - 1) * pageSize + 1;
  const endIndex = Math.min(safePage * pageSize, totalCount);
  const summary =
    totalCount === 0
      ? labels.empty
      : labels.showing
          .replace('{start}', String(startIndex))
          .replace('{end}', String(endIndex))
          .replace('{total}', String(totalCount));

  const canPrev = safePage > 1;
  const canNext = safePage < safeTotalPages;
  const buttonClass = cn(
    'inline-flex items-center gap-2 rounded-full border border-[var(--md-sys-color-outline-variant)] px-3 py-1 text-xs font-semibold',
    'bg-[var(--md-sys-color-surface-container)] text-[var(--md-sys-color-on-surface)] shadow-[var(--shadow-1)]',
    'transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)] disabled:opacity-50'
  );

  return (
    <div className="flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-xs">
      <span className="text-[var(--md-sys-color-on-surface-variant)]">{summary}</span>
      <div className="flex flex-wrap items-center gap-2">
        <button
          className={buttonClass}
          type="button"
          onClick={() => onPageChange(safePage - 1)}
          disabled={!canPrev || disabled}
        >
          <ChevronLeft className="h-3.5 w-3.5" />
          {labels.prev}
        </button>
        <label className="flex items-center gap-2 text-xs text-[var(--md-sys-color-on-surface-variant)]">
          {labels.page}
          <input
            className="w-16 rounded-full border border-[var(--md-sys-color-outline-variant)] bg-transparent px-2 py-1 text-center text-xs"
            type="number"
            min={1}
            max={safeTotalPages}
            value={safePage}
            disabled={disabled}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (!Number.isNaN(value)) {
                onPageChange(clamp(value, 1, safeTotalPages));
              }
            }}
          />
          <span>
            {labels.of} {safeTotalPages}
          </span>
        </label>
        <button
          className={buttonClass}
          type="button"
          onClick={() => onPageChange(safePage + 1)}
          disabled={!canNext || disabled}
        >
          {labels.next}
          <ChevronRight className="h-3.5 w-3.5" />
        </button>
      </div>
      <label className="flex items-center gap-2 text-xs text-[var(--md-sys-color-on-surface-variant)]">
        {labels.perPage}
        <select
          className="rounded-full border border-[var(--md-sys-color-outline-variant)] bg-transparent px-2 py-1 text-xs"
          value={pageSize}
          disabled={disabled}
          onChange={(event) => onPageSizeChange(Number(event.target.value))}
        >
          {sizes.map((size) => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
};

export function FilesPage({
  t,
  activeSection,
  isSectionCollapsed,
  toggleSection,
  screenshots,
  screenshotsLoading,
  takingScreenshot,
  loadScreenshots,
  takeScreenshot,
  downloadScreenshot,
  deleteScreenshot,
  updateScreenshotCaption,
  screenshotsPage,
  screenshotsPageSize,
  screenshotsTotal,
  screenshotsTotalPages,
  onScreenshotsPageChange,
  onScreenshotsPageSizeChange,
  currentPath,
  files,
  filesLoading,
  filesBusy,
  filesError,
  refreshFiles,
  navigateToDir,
  downloadFile,
  deleteFile,
  createDirectory,
  moveEntry,
  readFile,
  writeFile,
  uploadFiles,
  filesPage,
  filesPageSize,
  filesTotal,
  filesTotalPages,
  onFilesPageChange,
  onFilesPageSizeChange
}: FilesPageProps) {
  const [newFolderOpen, setNewFolderOpen] = useState(false);
  const [newFolderName, setNewFolderName] = useState('');
  const [renameTarget, setRenameTarget] = useState<FileEntry | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const [editorOpen, setEditorOpen] = useState(false);
  const [editorPath, setEditorPath] = useState('');
  const [editorContent, setEditorContent] = useState('');
  const [editorLoading, setEditorLoading] = useState(false);
  const [editorTruncated, setEditorTruncated] = useState(false);
  const [editorBinary, setEditorBinary] = useState(false);
  const [editorError, setEditorError] = useState('');
  const [dragActive, setDragActive] = useState(false);
  const [dragOverPath, setDragOverPath] = useState<string | null>(null);
  const [pathDraft, setPathDraft] = useState(currentPath);
  const [screenshotCaption, setScreenshotCaption] = useState('');
  const [selectedScreenshot, setSelectedScreenshot] = useState<Screenshot | null>(null);
  const [captionDraft, setCaptionDraft] = useState('');
  const [captionSaving, setCaptionSaving] = useState(false);

  const fileInputRef = useRef<HTMLInputElement | null>(null);
  const dragCounter = useRef(0);

  const sectionHighlightClass = (sectionId: string) =>
    activeSection === sectionId
      ? 'ring-2 ring-[var(--md-sys-color-primary)] ring-offset-2 ring-offset-[var(--md-sys-color-background)]'
      : '';
  const sectionToggleClassName = cn(
    'inline-flex items-center justify-center rounded-full border border-[var(--md-sys-color-outline-variant)] p-1.5',
    'bg-[var(--md-sys-color-surface-container)] text-[var(--md-sys-color-on-surface-variant)]',
    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
  );
  const sectionToggleIconClass = (collapsed: boolean) =>
    cn('h-4 w-4 transition-transform', collapsed ? '-rotate-90' : 'rotate-0');
  const downloadLabel = t('screenshot_download') || 'Download';
  const deleteLabel = t('screenshot_delete') || 'Delete';
  const captionLabel = t('screenshot_caption_label') || 'Caption';
  const captionPlaceholder = t('screenshot_caption_placeholder') || 'Add a caption...';
  const captionSaveLabel = t('screenshot_save_caption') || 'Save caption';
  const screenshotPreviewTitle = t('screenshot_preview_title') || 'Screenshot preview';
  const paginationLabels: PaginationLabels = {
    prev: t('pagination_prev') || 'Previous',
    next: t('pagination_next') || 'Next',
    page: t('pagination_page') || 'Page',
    of: t('pagination_of') || 'of',
    perPage: t('pagination_per_page') || 'Per page',
    showing: t('pagination_showing') || 'Showing {start}-{end} of {total}',
    empty: t('pagination_empty') || 'No items'
  };
  const captionDirty = Boolean(
    selectedScreenshot && captionDraft.trim() !== (selectedScreenshot.caption || '')
  );

  const toolbarButtonClass = cn(
    'inline-flex items-center gap-2 rounded-full border border-[var(--md-sys-color-outline-variant)]',
    'bg-[var(--md-sys-color-surface-container)] px-3 py-1.5 text-xs font-semibold',
    'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]',
    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--accent-outline)]'
  );
  const primaryButtonClass = cn(
    'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-3 py-1.5 text-xs font-semibold',
    'text-[var(--md-sys-color-on-primary)] shadow-[var(--shadow-1)] transition hover:-translate-y-0.5',
    'hover:shadow-[var(--shadow-2)] disabled:opacity-60'
  );
  const actionButtonClass = cn(
    'inline-flex h-7 w-7 items-center justify-center rounded-full border border-transparent',
    'text-[var(--md-sys-color-on-surface-variant)] transition hover:border-[var(--md-sys-color-outline-variant)]',
    'hover:bg-[var(--md-sys-color-surface-container-high)]'
  );

  const breadcrumbs = useMemo(() => {
    const segments = currentPath.split('/').filter(Boolean);
    const items = [{ label: '/', path: '/' }];
    let cursor = '';
    segments.forEach((segment) => {
      cursor = `${cursor}/${segment}`;
      items.push({ label: segment, path: cursor });
    });
    return items;
  }, [currentPath]);

  const parentPath = useMemo(() => getParentPath(currentPath), [currentPath]);

  useEffect(() => {
    setPathDraft(currentPath);
  }, [currentPath]);

  const openEditor = async (entry: FileEntry) => {
    setEditorOpen(true);
    setEditorPath(entry.path);
    setEditorLoading(true);
    setEditorTruncated(false);
    setEditorBinary(false);
    setEditorError('');
    try {
      const data = await readFile(entry.path);
      setEditorContent(data.content);
      setEditorTruncated(data.truncated);
      setEditorBinary(data.isBinary);
    } catch (error) {
      console.error('readFile error', error);
      setEditorError('Unable to load file contents.');
    } finally {
      setEditorLoading(false);
    }
  };

  const handleEditorSave = async () => {
    if (!editorPath || editorBinary) return;
    const saved = await writeFile(editorPath, editorContent);
    if (saved) {
      setEditorOpen(false);
      setEditorPath('');
      setEditorContent('');
      setEditorTruncated(false);
      setEditorBinary(false);
      setEditorError('');
    }
  };

  const confirmDelete = (entry: FileEntry) => {
    const kind = entry.is_dir ? 'folder' : 'file';
    if (!window.confirm(`Delete ${kind} "${entry.name}"?`)) return;
    void deleteFile(entry);
  };

  const openRename = (entry: FileEntry) => {
    setRenameTarget(entry);
    setRenameValue(entry.name);
  };

  const confirmRename = async () => {
    if (!renameTarget) return;
    const nextName = renameValue.trim();
    if (!nextName || nextName === renameTarget.name) {
      setRenameTarget(null);
      return;
    }
    const parent = getParentPath(renameTarget.path);
    const destination = joinPath(parent, nextName);
    await moveEntry(renameTarget.path, destination);
    setRenameTarget(null);
  };

  const confirmNewFolder = async () => {
    const name = newFolderName.trim();
    if (!name) return;
    await createDirectory(joinPath(currentPath, name));
    setNewFolderName('');
    setNewFolderOpen(false);
  };

  const handlePathSubmit = () => {
    const nextPath = pathDraft.trim();
    navigateToDir(nextPath || '/');
  };

  const openScreenshot = (shot: Screenshot) => {
    setSelectedScreenshot(shot);
    setCaptionDraft(shot.caption || '');
  };

  const closeScreenshot = () => {
    setSelectedScreenshot(null);
    setCaptionDraft('');
    setCaptionSaving(false);
  };

  const handleCaptionSave = async () => {
    if (!selectedScreenshot) return;
    const nextCaption = captionDraft.trim();
    setCaptionSaving(true);
    const saved = await updateScreenshotCaption(selectedScreenshot.id, nextCaption);
    if (saved) {
      setSelectedScreenshot({ ...selectedScreenshot, caption: nextCaption });
    }
    setCaptionSaving(false);
  };

  const handleScreenshotDelete = async () => {
    if (!selectedScreenshot) return;
    const deleted = await deleteScreenshot(selectedScreenshot.id);
    if (deleted) {
      closeScreenshot();
    }
  };

  const handleTakeScreenshot = async () => {
    const caption = screenshotCaption.trim();
    const saved = await takeScreenshot(caption);
    if (saved) {
      setScreenshotCaption('');
    }
  };

  const hasInternalDrag = (event: DragEvent) =>
    Array.from(event.dataTransfer.types || []).includes(INTERNAL_DRAG_TYPE);
  const hasFileDrag = (event: DragEvent) =>
    Array.from(event.dataTransfer.types || []).includes('Files');

  const handleDragEnter = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFileDrag(event)) return;
    dragCounter.current += 1;
    setDragActive(true);
  };

  const handleDragLeave = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFileDrag(event)) return;
    dragCounter.current -= 1;
    if (dragCounter.current <= 0) {
      setDragActive(false);
    }
  };

  const handleDragOver = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFileDrag(event)) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'copy';
  };

  const handleDrop = (event: DragEvent<HTMLDivElement>) => {
    if (hasInternalDrag(event)) {
      event.preventDefault();
      return;
    }
    event.preventDefault();
    dragCounter.current = 0;
    setDragActive(false);
    const dropped = event.dataTransfer.files;
    if (dropped && dropped.length) {
      void uploadFiles(dropped, currentPath);
    }
  };

  const handleEntryDragStart = (event: DragEvent<HTMLDivElement>, entry: FileEntry) => {
    event.dataTransfer.setData(INTERNAL_DRAG_TYPE, entry.path);
    event.dataTransfer.setData('text/plain', entry.path);
    event.dataTransfer.effectAllowed = 'move';
  };

  const handleFolderDragOver = (event: DragEvent<HTMLDivElement>, entry: FileEntry) => {
    if (hasInternalDrag(event)) {
      event.preventDefault();
      event.dataTransfer.dropEffect = 'move';
      setDragOverPath(entry.path);
      return;
    }
    if (hasFileDrag(event)) {
      event.preventDefault();
      event.dataTransfer.dropEffect = 'copy';
      setDragOverPath(entry.path);
    }
  };

  const handleFolderDrop = (event: DragEvent<HTMLDivElement>, entry: FileEntry) => {
    event.preventDefault();
    event.stopPropagation();
    const dropped = event.dataTransfer.files;
    const source = event.dataTransfer.getData(INTERNAL_DRAG_TYPE);
    setDragOverPath(null);
    if (dropped && dropped.length) {
      void uploadFiles(dropped, entry.path);
      return;
    }
    if (!source || source === entry.path) return;
    const name = basename(source);
    if (!name) return;
    const destination = joinPath(entry.path, name);
    if (destination === source) return;
    void moveEntry(source, destination);
  };

  return (
    <section className="flex flex-col gap-6">
      <article
        id="gallery"
        className={cn(
          'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
          'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)]',
          sectionHighlightClass('gallery')
        )}
        style={delayStyle(180)}
      >
        <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
          <div className="flex items-center gap-3">
            <Monitor className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
            <h2 className="font-display text-lg font-semibold">
              {t('section_gallery') || 'Screenshots'}
            </h2>
          </div>
          <button
            className={sectionToggleClassName}
            type="button"
            onClick={() => toggleSection('gallery')}
            aria-expanded={!isSectionCollapsed('gallery')}
          >
            <ChevronDown className={sectionToggleIconClass(isSectionCollapsed('gallery'))} />
          </button>
        </div>
        {!isSectionCollapsed('gallery') && (
          <div className="flex flex-col gap-4 px-6 pb-6">
            <div className="flex flex-wrap items-center gap-3">
              <button
                className={cn(
                  'inline-flex items-center gap-2 rounded-full bg-[var(--md-sys-color-primary)] px-4 py-2 text-sm font-semibold text-[var(--md-sys-color-on-primary)]',
                  'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]'
                )}
                type="button"
                onClick={() => void handleTakeScreenshot()}
                disabled={takingScreenshot}
              >
                <Monitor className="h-4 w-4" />
                {takingScreenshot ? 'Taking...' : 'Take Screenshot'}
              </button>
              <input
                className="min-w-[200px] flex-1 rounded-full border border-[var(--md-sys-color-outline-variant)] bg-transparent px-4 py-2 text-sm"
                type="text"
                value={screenshotCaption}
                onChange={(event) => setScreenshotCaption(event.target.value)}
                placeholder={captionPlaceholder}
              />
              <button
                className={cn(
                  'inline-flex items-center gap-2 rounded-full border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-4 py-2 text-sm font-semibold',
                  'shadow-[var(--shadow-1)] transition hover:-translate-y-0.5 hover:shadow-[var(--shadow-2)]'
                )}
                type="button"
                onClick={() => void loadScreenshots()}
                disabled={screenshotsLoading}
              >
                <RefreshCw className={cn('h-4 w-4', screenshotsLoading && 'animate-spin')} />
                Refresh
              </button>
            </div>
            {screenshots.length > 0 ? (
              <div className="flex flex-col gap-3">
                <div className="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-5">
                  {screenshots.map((ss) => (
                    <div
                      key={ss.id}
                      className="group relative aspect-video overflow-hidden rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] transition hover:border-[var(--accent-border)]"
                    >
                      <button
                        type="button"
                        className="block h-full w-full"
                        onClick={() => openScreenshot(ss)}
                      >
                        <img
                          src={`${API_BASE}/api/screenshots/${ss.id}`}
                          alt={ss.caption || ss.filename}
                          className="h-full w-full object-cover"
                        />
                      </button>
                      <div className="pointer-events-none absolute inset-0 flex items-end bg-gradient-to-t from-black/60 via-black/20 to-transparent p-2">
                        <div className="flex w-full items-center justify-between gap-2">
                          <p className="truncate text-xs text-white">{ss.caption || ss.filename}</p>
                          <div className="pointer-events-auto flex items-center gap-2 opacity-0 transition group-hover:opacity-100">
                            <a
                              className="inline-flex h-7 w-7 items-center justify-center rounded-full bg-black/50 text-white transition hover:bg-black/70"
                              href="#"
                              title={downloadLabel}
                              aria-label={downloadLabel}
                              onClick={(event) => {
                                event.preventDefault();
                                event.stopPropagation();
                                void downloadScreenshot(ss.id, ss.filename);
                              }}
                            >
                              <Download className="h-4 w-4" />
                            </a>
                            <button
                              className="inline-flex h-7 w-7 items-center justify-center rounded-full bg-black/50 text-white transition hover:bg-black/70 disabled:opacity-50"
                              type="button"
                              onClick={(event) => {
                                event.stopPropagation();
                                void deleteScreenshot(ss.id);
                              }}
                              title={deleteLabel}
                              aria-label={deleteLabel}
                              disabled={screenshotsLoading}
                            >
                              <Trash2 className="h-4 w-4" />
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
                <PaginationBar
                  page={screenshotsPage}
                  pageSize={screenshotsPageSize}
                  totalPages={screenshotsTotalPages}
                  totalCount={screenshotsTotal}
                  sizes={[12, 24, 48]}
                  labels={paginationLabels}
                  disabled={screenshotsLoading}
                  onPageChange={onScreenshotsPageChange}
                  onPageSizeChange={onScreenshotsPageSizeChange}
                />
              </div>
            ) : (
              <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">
                No screenshots yet. Take one!
              </p>
            )}
          </div>
        )}
      </article>

      <article
        id="file-manager"
        className={cn(
          'reveal flex flex-col gap-3 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] shadow-[var(--shadow-1)]',
          'transition duration-200 ease-out hover:-translate-y-1 hover:shadow-[var(--shadow-2)] hover:border-[var(--accent-border)]',
          sectionHighlightClass('file-manager')
        )}
        style={delayStyle(220)}
      >
        <div className="flex flex-wrap items-center justify-between gap-3 px-6 pb-4 pt-6 text-[var(--md-sys-color-primary)]">
          <div className="flex items-center gap-3">
            <FolderOpen className="h-7 w-7 rounded-[16px] bg-[var(--md-sys-color-primary-container)] p-1.5 text-[var(--md-sys-color-on-primary-container)]" />
            <h2 className="font-display text-lg font-semibold">
              {t('section_file_manager') || 'File Manager'}
            </h2>
          </div>
          <button
            className={sectionToggleClassName}
            type="button"
            onClick={() => toggleSection('file-manager')}
            aria-expanded={!isSectionCollapsed('file-manager')}
          >
            <ChevronDown className={sectionToggleIconClass(isSectionCollapsed('file-manager'))} />
          </button>
        </div>
        {!isSectionCollapsed('file-manager') && (
          <div className="flex flex-col gap-4 px-6 pb-6">
            <div className="flex flex-wrap items-center gap-2">
              <button
                className={toolbarButtonClass}
                type="button"
                onClick={() => navigateToDir('/sdcard')}
              >
                /sdcard
              </button>
              <button
                className={toolbarButtonClass}
                type="button"
                onClick={() => navigateToDir('/sdcard/DCIM')}
              >
                DCIM
              </button>
              <button
                className={toolbarButtonClass}
                type="button"
                onClick={() => navigateToDir('/sdcard/Download')}
              >
                Download
              </button>
              <div className="flex flex-1 items-center gap-2 min-w-[200px]">
                <input
                  className="w-full rounded-full border border-[var(--md-sys-color-outline-variant)] bg-transparent px-3 py-1.5 text-xs"
                  type="text"
                  value={pathDraft}
                  onChange={(event) => setPathDraft(event.target.value)}
                  placeholder="/sdcard"
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') {
                      handlePathSubmit();
                    }
                  }}
                />
                <button
                  className={toolbarButtonClass}
                  type="button"
                  onClick={handlePathSubmit}
                >
                  <ArrowRight className="h-3.5 w-3.5" />
                  Go
                </button>
              </div>
              <button
                className={toolbarButtonClass}
                type="button"
                onClick={() => navigateToDir(parentPath)}
                disabled={currentPath === '/'}
                title="Up"
              >
                <ArrowUp className="h-3.5 w-3.5" />
                Up
              </button>
              <button
                className={toolbarButtonClass}
                type="button"
                onClick={() => void refreshFiles()}
                disabled={filesLoading}
              >
                <RefreshCw className={cn('h-3.5 w-3.5', filesLoading && 'animate-spin')} />
                Refresh
              </button>
              <button
                className={primaryButtonClass}
                type="button"
                onClick={() => fileInputRef.current?.click()}
                disabled={filesBusy}
              >
                <Upload className="h-3.5 w-3.5" />
                Upload
              </button>
              <button
                className={toolbarButtonClass}
                type="button"
                onClick={() => setNewFolderOpen(true)}
                disabled={filesBusy}
              >
                <FolderPlus className="h-3.5 w-3.5" />
                New Folder
              </button>
              <span className="ml-auto text-xs text-[var(--md-sys-color-on-surface-variant)]">
                {currentPath}
              </span>
            </div>

            <div className="flex flex-wrap items-center gap-2 rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-3 py-2 text-xs">
              {breadcrumbs.map((crumb, index) => (
                <button
                  key={crumb.path}
                  type="button"
                  className={cn(
                    'rounded-full px-2 py-1 transition',
                    crumb.path === currentPath
                      ? 'bg-[var(--md-sys-color-primary-container)] text-[var(--md-sys-color-on-primary-container)]'
                      : 'text-[var(--md-sys-color-on-surface-variant)] hover:bg-[var(--md-sys-color-surface-container-high)]'
                  )}
                  onClick={() => navigateToDir(crumb.path)}
                  disabled={crumb.path === currentPath}
                >
                  {index === 0 ? '/' : crumb.label}
                </button>
              ))}
            </div>

            <input
              ref={fileInputRef}
              type="file"
              multiple
              className="hidden"
              onChange={(event) => {
                if (event.target.files?.length) {
                  void uploadFiles(event.target.files, currentPath);
                  event.target.value = '';
                }
              }}
            />

            {filesError && (
              <div className="rounded-[var(--radius-sm)] border border-red-500/40 bg-red-500/10 px-4 py-2 text-xs text-red-200">
                {filesError}
              </div>
            )}

            {filesLoading ? (
              <div className="flex items-center justify-center py-8">
                <div className="loading-spinner" />
              </div>
            ) : files.length > 0 ? (
              <div className="flex flex-col gap-3">
                <div
                  className="relative flex flex-col gap-1 rounded-[var(--radius-sm)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] overflow-hidden"
                  onDragEnter={handleDragEnter}
                  onDragLeave={handleDragLeave}
                  onDragOver={handleDragOver}
                  onDrop={handleDrop}
                >
                  {dragActive && (
                    <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center border-2 border-dashed border-[var(--md-sys-color-primary)] bg-[var(--md-sys-color-primary-container)]/60 text-sm font-semibold text-[var(--md-sys-color-on-primary-container)]">
                      Drop files to upload
                    </div>
                  )}
                  {currentPath !== '/' && (
                    <div className="flex items-center gap-3 px-4 py-2 text-sm">
                      <button
                        className="flex items-center gap-3 text-left hover:text-[var(--md-sys-color-primary)]"
                        type="button"
                        onClick={() => navigateToDir(parentPath)}
                      >
                        <Folder className="h-4 w-4 text-[var(--md-sys-color-primary)]" />
                        <span>..</span>
                      </button>
                    </div>
                  )}
                  {files.map((file) => {
                    const isDropTarget = dragOverPath === file.path;
                    return (
                      <div
                        key={file.path}
                        className={cn(
                          'group flex items-center gap-3 px-4 py-2 text-sm transition',
                          'hover:bg-[var(--md-sys-color-surface-container-high)]',
                          isDropTarget && 'ring-2 ring-[var(--md-sys-color-primary)]'
                        )}
                        draggable
                        onDragStart={(event) => handleEntryDragStart(event, file)}
                        onDragOver={(event) => file.is_dir && handleFolderDragOver(event, file)}
                        onDrop={(event) => file.is_dir && handleFolderDrop(event, file)}
                        onDragLeave={() => file.is_dir && setDragOverPath(null)}
                      >
                        <button
                          className={cn(
                            'flex min-w-0 flex-1 items-center gap-3 text-left',
                            file.is_dir && 'hover:text-[var(--md-sys-color-primary)]'
                          )}
                          type="button"
                          onClick={() => file.is_dir && navigateToDir(file.path)}
                          disabled={!file.is_dir}
                        >
                          {file.is_dir ? (
                            <Folder className="h-4 w-4 text-[var(--md-sys-color-primary)]" />
                          ) : (
                            <FileText className="h-4 w-4 text-[var(--md-sys-color-on-surface-variant)]" />
                          )}
                          <span className="truncate">{file.name}</span>
                        </button>
                        <span className="hidden text-xs text-[var(--md-sys-color-on-surface-variant)] sm:block">
                          {file.is_dir ? '-' : formatBytes(file.size)}
                        </span>
                        <span className="hidden text-xs text-[var(--md-sys-color-on-surface-variant)] md:block">
                          {file.date || '-'}
                        </span>
                        <div className="flex items-center gap-1 opacity-0 transition group-hover:opacity-100">
                          {!file.is_dir && (
                            <button
                              className={actionButtonClass}
                              type="button"
                              title="Download"
                              onClick={() => void downloadFile(file)}
                            >
                              <Download className="h-3.5 w-3.5" />
                            </button>
                          )}
                          {!file.is_dir && (
                            <button
                              className={actionButtonClass}
                              type="button"
                              title="Edit"
                              onClick={() => void openEditor(file)}
                            >
                              <FileText className="h-3.5 w-3.5" />
                            </button>
                          )}
                          <button
                            className={actionButtonClass}
                            type="button"
                            title="Rename"
                            onClick={() => openRename(file)}
                          >
                            <Pencil className="h-3.5 w-3.5" />
                          </button>
                          <button
                            className={actionButtonClass}
                            type="button"
                            title="Delete"
                            onClick={() => confirmDelete(file)}
                          >
                            <Trash2 className="h-3.5 w-3.5" />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </div>
                <PaginationBar
                  page={filesPage}
                  pageSize={filesPageSize}
                  totalPages={filesTotalPages}
                  totalCount={filesTotal}
                  sizes={[25, 50, 100, 200]}
                  labels={paginationLabels}
                  disabled={filesLoading}
                  onPageChange={onFilesPageChange}
                  onPageSizeChange={onFilesPageSizeChange}
                />
              </div>
            ) : (
              <div
                className="flex flex-col items-center justify-center gap-3 rounded-[var(--radius-sm)] border border-dashed border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] px-6 py-8 text-sm text-[var(--md-sys-color-on-surface-variant)]"
                onDragEnter={handleDragEnter}
                onDragLeave={handleDragLeave}
                onDragOver={handleDragOver}
                onDrop={handleDrop}
              >
                <p>Drop files here to upload or pick a folder.</p>
                <button
                  className={primaryButtonClass}
                  type="button"
                  onClick={() => fileInputRef.current?.click()}
                  disabled={filesBusy}
                >
                  <Upload className="h-3.5 w-3.5" />
                  Upload files
                </button>
              </div>
            )}
          </div>
        )}
      </article>

      <div
        className={cn(
          'modal-overlay fixed inset-0 z-[1200] flex items-center justify-center p-5 transition-opacity',
          newFolderOpen ? 'pointer-events-auto opacity-100' : 'pointer-events-none opacity-0'
        )}
        aria-hidden={!newFolderOpen}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            setNewFolderOpen(false);
          }
        }}
      >
        <div
          className="relative flex w-full max-w-[480px] flex-col gap-4 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] p-6 shadow-[var(--shadow-3)]"
          role="dialog"
          aria-modal="true"
          aria-labelledby="newFolderTitle"
        >
          <button
            className="absolute right-4 top-4 text-[var(--md-sys-color-on-surface-variant)]"
            onClick={() => setNewFolderOpen(false)}
            aria-label="Close"
          >
            <X className="h-5 w-5" />
          </button>
          <div className="flex items-center gap-3 text-[var(--md-sys-color-primary)]">
            <FolderPlus className="h-6 w-6 rounded-[14px] bg-[var(--md-sys-color-primary-container)] p-2 text-[var(--md-sys-color-on-primary-container)]" />
            <h2 id="newFolderTitle" className="font-display text-lg font-semibold">
              New Folder
            </h2>
          </div>
          <label className="flex flex-col gap-2 text-sm">
            <span className="text-[var(--md-sys-color-on-surface-variant)]">Folder name</span>
            <input
              className="w-full rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-transparent px-3 py-2 text-sm"
              type="text"
              value={newFolderName}
              onChange={(event) => setNewFolderName(event.target.value)}
              placeholder="New folder"
            />
          </label>
          <div className="flex flex-wrap justify-end gap-3">
            <button
              className={toolbarButtonClass}
              type="button"
              onClick={() => setNewFolderOpen(false)}
            >
              Cancel
            </button>
            <button className={primaryButtonClass} type="button" onClick={() => void confirmNewFolder()}>
              Create
            </button>
          </div>
        </div>
      </div>

      <div
        className={cn(
          'modal-overlay fixed inset-0 z-[1200] flex items-center justify-center p-5 transition-opacity',
          renameTarget ? 'pointer-events-auto opacity-100' : 'pointer-events-none opacity-0'
        )}
        aria-hidden={!renameTarget}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            setRenameTarget(null);
          }
        }}
      >
        <div
          className="relative flex w-full max-w-[480px] flex-col gap-4 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] p-6 shadow-[var(--shadow-3)]"
          role="dialog"
          aria-modal="true"
          aria-labelledby="renameTitle"
        >
          <button
            className="absolute right-4 top-4 text-[var(--md-sys-color-on-surface-variant)]"
            onClick={() => setRenameTarget(null)}
            aria-label="Close"
          >
            <X className="h-5 w-5" />
          </button>
          <div className="flex items-center gap-3 text-[var(--md-sys-color-primary)]">
            <Pencil className="h-6 w-6 rounded-[14px] bg-[var(--md-sys-color-primary-container)] p-2 text-[var(--md-sys-color-on-primary-container)]" />
            <h2 id="renameTitle" className="font-display text-lg font-semibold">
              Rename
            </h2>
          </div>
          <label className="flex flex-col gap-2 text-sm">
            <span className="text-[var(--md-sys-color-on-surface-variant)]">New name</span>
            <input
              className="w-full rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-transparent px-3 py-2 text-sm"
              type="text"
              value={renameValue}
              onChange={(event) => setRenameValue(event.target.value)}
            />
          </label>
          <div className="flex flex-wrap justify-end gap-3">
            <button
              className={toolbarButtonClass}
              type="button"
              onClick={() => setRenameTarget(null)}
            >
              Cancel
            </button>
            <button className={primaryButtonClass} type="button" onClick={() => void confirmRename()}>
              Rename
            </button>
          </div>
        </div>
      </div>

      <div
        className={cn(
          'modal-overlay fixed inset-0 z-[1200] flex items-center justify-center p-5 transition-opacity',
          editorOpen ? 'pointer-events-auto opacity-100' : 'pointer-events-none opacity-0'
        )}
        aria-hidden={!editorOpen}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            setEditorOpen(false);
          }
        }}
      >
        <div
          className="relative flex w-full max-w-[780px] flex-col gap-4 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] p-6 shadow-[var(--shadow-3)]"
          role="dialog"
          aria-modal="true"
          aria-labelledby="editorTitle"
        >
          <button
            className="absolute right-4 top-4 text-[var(--md-sys-color-on-surface-variant)]"
            onClick={() => setEditorOpen(false)}
            aria-label="Close"
          >
            <X className="h-5 w-5" />
          </button>
          <div className="flex items-center gap-3 text-[var(--md-sys-color-primary)]">
            <FileText className="h-6 w-6 rounded-[14px] bg-[var(--md-sys-color-primary-container)] p-2 text-[var(--md-sys-color-on-primary-container)]" />
            <h2 id="editorTitle" className="font-display text-lg font-semibold">
              Edit: {basename(editorPath)}
            </h2>
          </div>
          {editorLoading ? (
            <div className="flex items-center gap-2 text-sm text-[var(--md-sys-color-on-surface-variant)]">
              <RefreshCw className="h-4 w-4 animate-spin" />
              Loading file contents...
            </div>
          ) : editorError ? (
            <p className="text-sm text-red-500">{editorError}</p>
          ) : editorBinary ? (
            <p className="text-sm text-[var(--md-sys-color-on-surface-variant)]">
              Binary file detected. Use download to edit locally.
            </p>
          ) : (
            <>
              {editorTruncated && (
                <p className="text-xs text-[var(--md-sys-color-on-surface-variant)]">
                  Showing the first chunk of the file. Download for full editing.
                </p>
              )}
              <textarea
                className="min-h-[320px] w-full resize-y rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-transparent p-3 text-sm"
                value={editorContent}
                onChange={(event) => setEditorContent(event.target.value)}
              />
            </>
          )}
          <div className="flex flex-wrap justify-end gap-3">
            <button className={toolbarButtonClass} type="button" onClick={() => setEditorOpen(false)}>
              Close
            </button>
            <button
              className={primaryButtonClass}
              type="button"
              onClick={() => void handleEditorSave()}
              disabled={editorLoading || editorBinary || filesBusy}
            >
              Save
            </button>
          </div>
        </div>
      </div>

      <div
        className={cn(
          'modal-overlay fixed inset-0 z-[1200] flex items-center justify-center p-5 transition-opacity',
          selectedScreenshot ? 'pointer-events-auto opacity-100' : 'pointer-events-none opacity-0'
        )}
        aria-hidden={!selectedScreenshot}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            closeScreenshot();
          }
        }}
      >
        <div
          className="relative flex w-full max-w-[980px] flex-col gap-4 rounded-[var(--radius-lg)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface)] p-6 shadow-[var(--shadow-3)]"
          role="dialog"
          aria-modal="true"
          aria-labelledby="screenshotTitle"
        >
          <button
            className="absolute right-4 top-4 text-[var(--md-sys-color-on-surface-variant)]"
            onClick={closeScreenshot}
            aria-label="Close"
          >
            <X className="h-5 w-5" />
          </button>
          <div className="flex items-center gap-3 text-[var(--md-sys-color-primary)]">
            <Monitor className="h-6 w-6 rounded-[14px] bg-[var(--md-sys-color-primary-container)] p-2 text-[var(--md-sys-color-on-primary-container)]" />
            <h2 id="screenshotTitle" className="font-display text-lg font-semibold">
              {screenshotPreviewTitle}
            </h2>
          </div>
          {selectedScreenshot && (
            <>
              <div className="overflow-hidden rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-[var(--md-sys-color-surface-container)] p-2">
                <img
                  src={`${API_BASE}/api/screenshots/${selectedScreenshot.id}`}
                  alt={selectedScreenshot.caption || selectedScreenshot.filename}
                  className="max-h-[60vh] w-full object-contain"
                />
              </div>
              <label className="flex flex-col gap-2 text-sm">
                <span className="text-[var(--md-sys-color-on-surface-variant)]">{captionLabel}</span>
                <input
                  className="w-full rounded-[var(--radius-md)] border border-[var(--md-sys-color-outline-variant)] bg-transparent px-3 py-2 text-sm"
                  type="text"
                  value={captionDraft}
                  onChange={(event) => setCaptionDraft(event.target.value)}
                  placeholder={captionPlaceholder}
                />
              </label>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <span className="text-xs text-[var(--md-sys-color-on-surface-variant)]">
                  {selectedScreenshot.filename}
                </span>
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    className={toolbarButtonClass}
                    type="button"
                    onClick={() =>
                      void downloadScreenshot(selectedScreenshot.id, selectedScreenshot.filename)
                    }
                  >
                    <Download className="h-3.5 w-3.5" />
                    {downloadLabel}
                  </button>
                  <button
                    className={toolbarButtonClass}
                    type="button"
                    onClick={() => void handleScreenshotDelete()}
                    disabled={screenshotsLoading}
                  >
                    <Trash2 className="h-3.5 w-3.5" />
                    {deleteLabel}
                  </button>
                  <button
                    className={primaryButtonClass}
                    type="button"
                    onClick={() => void handleCaptionSave()}
                    disabled={captionSaving || !captionDirty}
                  >
                    <Pencil className="h-3.5 w-3.5" />
                    {captionSaveLabel}
                  </button>
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    </section>
  );
}
