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
import { screenshotSrc } from '../../lib/assets';
import { confirmAction } from '../../lib/dialogs';
import { delayStyle } from '../../lib/style';
import { cn } from '../../utils';
import type { FileEntry, Screenshot } from '../../types/app';

type FilesPageProps = {
  t: (key: string) => string;
  formatMessage: (key: string, params?: Record<string, string>) => string;
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
  /**
   * Выбор файлов средствами системы: содержимое `<input type=file>` до бэкенда
   * не доедет, ему нужны пути на диске.
   */
  pickUpload: (destination: string) => void | Promise<void>;
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
  const buttonClass = 'm3-btn m3-state m3-btn--outlined m3-btn--xs';

  return (
    <div className="m3-row text-xs">
      <span className="text-on-surface-variant">{summary}</span>
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
        <label className="flex items-center gap-2 m3-body-small m3-on-variant">
          {labels.page}
          <input
            className="m3-field m3-field--sm w-16 text-center"
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
      <label className="flex items-center gap-2 m3-body-small m3-on-variant">
        {labels.perPage}
        <select
          className="m3-field m3-select m3-field--sm w-auto"
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
  formatMessage,
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
  pickUpload,
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

  const dragCounter = useRef(0);

  const sectionHighlightClass = (sectionId: string) =>
    activeSection === sectionId ? 'm3-panel--active' : '';
  const sectionToggleClassName = 'm3-icon-btn m3-state m3-icon-btn--sm m3-icon-btn--outlined';
  const sectionToggleIconClass = (collapsed: boolean) =>
    cn('m3-panel__toggle-icon', collapsed && 'm3-panel__toggle-icon--collapsed');
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

  const toolbarButtonClass = 'm3-btn m3-state m3-btn--elevated m3-btn--xs';
  const primaryButtonClass = 'm3-btn m3-state m3-btn--filled m3-btn--xs';
  const actionButtonClass = 'm3-icon-btn m3-state m3-icon-btn--sm';

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

  const confirmDelete = async (entry: FileEntry) => {
    const kind = t(entry.is_dir ? 'files_kind_folder' : 'files_kind_file');
    const accepted = await confirmAction(
      formatMessage('files_delete_confirm', { kind, name: entry.name })
    );
    if (!accepted) return;
    void deleteFile(entry);
  };

  // Раньше клик по корзине в плитке галереи удалял скриншот без вопросов.
  const confirmDeleteScreenshot = async (id: string) => {
    if (!(await confirmAction(t('files_screenshot_delete_confirm')))) return;
    void deleteScreenshot(id);
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
    const source = event.dataTransfer.getData(INTERNAL_DRAG_TYPE);
    setDragOverPath(null);
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
          'm3-enter m3-panel',
          sectionHighlightClass('gallery')
        )}
        style={delayStyle(180)}
      >
        <div className="m3-panel__header">
          <div className="flex items-center gap-3">
            <Monitor className="m3-token" />
            <h2 className="m3-panel__title">
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
          <div className="m3-panel__body">
            <div className="flex flex-wrap items-center gap-3">
              <button
                className="m3-btn m3-state m3-btn--filled m3-btn--sm"
                type="button"
                onClick={() => void handleTakeScreenshot()}
                disabled={takingScreenshot}
              >
                <Monitor className="h-4 w-4" />
                {takingScreenshot ? t('files_taking') : t('files_take_screenshot')}
              </button>
              <input
                className="m3-field min-w-[200px] flex-1"
                type="text"
                value={screenshotCaption}
                onChange={(event) => setScreenshotCaption(event.target.value)}
                placeholder={captionPlaceholder}
              />
              <button
                className="m3-btn m3-state m3-btn--elevated m3-btn--sm"
                type="button"
                onClick={() => void loadScreenshots()}
                disabled={screenshotsLoading}
              >
                <RefreshCw className={cn('h-4 w-4', screenshotsLoading && 'animate-spin')} />
                {t('files_refresh')}
              </button>
            </div>
            {screenshots.length > 0 ? (
              <div className="flex flex-col gap-3">
                <div className="grid grid-cols-3 gap-3 sm:grid-cols-4 md:grid-cols-5">
                  {screenshots.map((ss) => (
                    <div
                      key={ss.id}
                      className="group relative aspect-video overflow-hidden rounded-m3md bg-surface-container ring-1 ring-outline-variant transition hover:ring-2 hover:ring-primary"
                    >
                      <button
                        type="button"
                        className="block h-full w-full"
                        onClick={() => openScreenshot(ss)}
                      >
                        <img
                          src={screenshotSrc(ss)}
                          alt={ss.caption || ss.filename}
                          className="h-full w-full object-cover"
                          /* Плитки грузятся по мере прокрутки: скриншот весит
                             мегабайты, и страница из сорока штук иначе
                             декодировала бы их все разом. */
                          loading="lazy"
                          decoding="async"
                          draggable={false}
                        />
                      </button>
                      <div className="pointer-events-none absolute inset-0 flex items-end bg-gradient-to-t from-black/60 via-black/20 to-transparent p-2">
                        <div className="flex w-full items-center justify-between gap-2">
                          <p className="truncate text-xs text-white">{ss.caption || ss.filename}</p>
                          <div className="pointer-events-auto flex items-center gap-2 opacity-0 transition group-hover:opacity-100">
                            <a
                              className="m3-icon-btn m3-state m3-icon-btn--sm bg-black/55 text-white"
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
                              className="m3-icon-btn m3-state m3-icon-btn--sm bg-black/55 text-white"
                              type="button"
                              onClick={(event) => {
                                event.stopPropagation();
                                void confirmDeleteScreenshot(ss.id);
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
              <p className="m3-body-medium m3-on-variant">
                {t('files_no_screenshots')}
              </p>
            )}
          </div>
        )}
      </article>

      <article
        id="file-manager"
        className={cn(
          'm3-enter m3-panel',
          sectionHighlightClass('file-manager')
        )}
        style={delayStyle(220)}
      >
        <div className="m3-panel__header">
          <div className="flex items-center gap-3">
            <FolderOpen className="m3-token" />
            <h2 className="m3-panel__title">
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
          <div className="m3-panel__body">
            {/*
              Панель в две строки, а не в одну.
              Раньше десять элементов стояли подряд: адрес, переходы, действия
              и ещё раз текущий путь в конце. При обычной ширине окна хвост
              уезжал за край секции, а глазу было не за что зацепиться — всё
              выглядело одинаково важным. Теперь сверху адрес и навигация,
              снизу — быстрые папки и действия над содержимым.
            */}
            <div className="flex flex-wrap items-center gap-2">
              <input
                className="m3-field m3-field--sm min-w-[180px] flex-1"
                type="text"
                value={pathDraft}
                onChange={(event) => setPathDraft(event.target.value)}
                placeholder="/sdcard"
                aria-label={t('files_go')}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    handlePathSubmit();
                  }
                }}
              />
              <button className={primaryButtonClass} type="button" onClick={handlePathSubmit}>
                <ArrowRight className="h-3.5 w-3.5" />
                {t('files_go')}
              </button>
              <button
                className={toolbarButtonClass}
                type="button"
                onClick={() => navigateToDir(parentPath)}
                disabled={currentPath === '/'}
                title={t('files_up')}
              >
                <ArrowUp className="h-3.5 w-3.5" />
                {t('files_up')}
              </button>
              <button
                className={toolbarButtonClass}
                type="button"
                onClick={() => void refreshFiles()}
                disabled={filesLoading}
              >
                <RefreshCw className={cn('h-3.5 w-3.5', filesLoading && 'animate-spin')} />
                {t('files_refresh')}
              </button>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              {['/sdcard', '/sdcard/DCIM', '/sdcard/Download'].map((shortcut) => (
                <button
                  key={shortcut}
                  className={cn(
                    'm3-chip m3-state m3-chip--mono',
                    currentPath === shortcut && 'm3-chip--selected'
                  )}
                  type="button"
                  onClick={() => navigateToDir(shortcut)}
                >
                  {shortcut === '/sdcard' ? shortcut : basename(shortcut)}
                </button>
              ))}
              <span className="flex-1" />
              <button
                className={toolbarButtonClass}
                type="button"
                onClick={() => setNewFolderOpen(true)}
                disabled={filesBusy}
              >
                <FolderPlus className="h-3.5 w-3.5" />
                {t('files_new_folder')}
              </button>
              <button
                className={primaryButtonClass}
                type="button"
                onClick={() => void pickUpload(currentPath)}
                disabled={filesBusy}
              >
                <Upload className="h-3.5 w-3.5" />
                {t('files_upload')}
              </button>
            </div>

            <div className="m3-row justify-start text-xs">
              {breadcrumbs.map((crumb, index) => (
                <button
                  key={crumb.path}
                  type="button"
                  className={cn(
                    'rounded-full px-2 py-1 transition',
                    crumb.path === currentPath
                      ? 'bg-primary-container text-on-primary-container'
                      : 'text-on-surface-variant hover:bg-surface-high'
                  )}
                  onClick={() => navigateToDir(crumb.path)}
                  disabled={crumb.path === currentPath}
                >
                  {index === 0 ? '/' : crumb.label}
                </button>
              ))}
            </div>

            {filesError && (
              <div className="rounded-m3md bg-error-container px-4 py-2 text-xs text-on-error-container">
                {filesError}
              </div>
            )}

            {filesLoading ? (
              <div className="flex items-center justify-center py-8">
                <div className="m3-spinner" />
              </div>
            ) : files.length > 0 ? (
              <div className="flex flex-col gap-3">
                <div
                  className="relative flex flex-col gap-1 overflow-hidden m3-tray p-0"
                  onDragEnter={handleDragEnter}
                  onDragLeave={handleDragLeave}
                  onDragOver={handleDragOver}
                  onDrop={handleDrop}
                >
                  {dragActive && (
                    <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center rounded-m3lg border-2 border-dashed border-primary bg-primary-container/70 text-sm font-semibold text-on-primary-container">
                      {t('files_drop_hint')}
                    </div>
                  )}
                  {currentPath !== '/' && (
                    <div className="flex items-center gap-3 px-4 py-2 text-sm">
                      <button
                        className="flex items-center gap-3 text-left hover:text-primary"
                        type="button"
                        onClick={() => navigateToDir(parentPath)}
                      >
                        <Folder className="h-4 w-4 text-primary" />
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
                          'group m3-cv-line flex items-center gap-3 rounded-m3sm px-4 py-2 text-sm',
                          'transition-colors duration-short hover:bg-surface-high',
                          isDropTarget && 'ring-2 ring-inset ring-primary'
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
                            file.is_dir && 'hover:text-primary'
                          )}
                          type="button"
                          onClick={() => file.is_dir && navigateToDir(file.path)}
                          disabled={!file.is_dir}
                        >
                          {file.is_dir ? (
                            <Folder className="h-4 w-4 text-primary" />
                          ) : (
                            <FileText className="h-4 w-4 text-on-surface-variant" />
                          )}
                          <span className="truncate">{file.name}</span>
                        </button>
                        <span className="hidden m3-body-small m3-on-variant sm:block">
                          {file.is_dir ? '-' : formatBytes(file.size)}
                        </span>
                        <span className="hidden m3-body-small m3-on-variant md:block">
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
                            title={t('files_rename')}
                            onClick={() => openRename(file)}
                          >
                            <Pencil className="h-3.5 w-3.5" />
                          </button>
                          <button
                            className={actionButtonClass}
                            type="button"
                            title={t('screenshot_delete')}
                            onClick={() => void confirmDelete(file)}
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
                className="m3-empty"
                onDragEnter={handleDragEnter}
                onDragLeave={handleDragLeave}
                onDragOver={handleDragOver}
                onDrop={handleDrop}
              >
                <p>{t('files_drop_hint')}</p>
                <button
                  className={primaryButtonClass}
                  type="button"
                  onClick={() => void pickUpload(currentPath)}
                  disabled={filesBusy}
                >
                  <Upload className="h-3.5 w-3.5" />
                  {t('files_upload_files')}
                </button>
              </div>
            )}
          </div>
        )}
      </article>

      <div
        className={cn(
          'm3-scrim z-[1200] flex items-center justify-center p-5 transition-opacity',
          newFolderOpen ? 'pointer-events-auto opacity-100' : 'pointer-events-none invisible opacity-0'
        )}
        aria-hidden={!newFolderOpen}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            setNewFolderOpen(false);
          }
        }}
      >
        <div
          className="m3-dialog m3-dialog--enter relative max-w-[480px]"
          role="dialog"
          aria-modal="true"
          aria-labelledby="newFolderTitle"
        >
          <button
            className="m3-icon-btn m3-state m3-icon-btn--sm absolute right-4 top-4"
            onClick={() => setNewFolderOpen(false)}
            aria-label={t('files_close')}
          >
            <X className="h-5 w-5" />
          </button>
          <div className="flex items-center gap-3 text-primary">
            <FolderPlus className="m3-token m3-token--sm" />
            <h2 id="newFolderTitle" className="m3-panel__title">
              {t('files_new_folder')}
            </h2>
          </div>
          <label className="flex flex-col gap-2 text-sm">
            <span className="text-on-surface-variant">{t('files_folder_name')}</span>
            <input
              className="m3-field"
              type="text"
              value={newFolderName}
              onChange={(event) => setNewFolderName(event.target.value)}
              placeholder={t('files_folder_name')}
            />
          </label>
          <div className="flex flex-wrap justify-end gap-3">
            <button
              className={toolbarButtonClass}
              type="button"
              onClick={() => setNewFolderOpen(false)}
            >
              {t('files_cancel')}
            </button>
            <button className={primaryButtonClass} type="button" onClick={() => void confirmNewFolder()}>
              {t('files_create')}
            </button>
          </div>
        </div>
      </div>

      <div
        className={cn(
          'm3-scrim z-[1200] flex items-center justify-center p-5 transition-opacity',
          renameTarget ? 'pointer-events-auto opacity-100' : 'pointer-events-none invisible opacity-0'
        )}
        aria-hidden={!renameTarget}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            setRenameTarget(null);
          }
        }}
      >
        <div
          className="m3-dialog m3-dialog--enter relative max-w-[480px]"
          role="dialog"
          aria-modal="true"
          aria-labelledby="renameTitle"
        >
          <button
            className="m3-icon-btn m3-state m3-icon-btn--sm absolute right-4 top-4"
            onClick={() => setRenameTarget(null)}
            aria-label={t('files_close')}
          >
            <X className="h-5 w-5" />
          </button>
          <div className="flex items-center gap-3 text-primary">
            <Pencil className="m3-token m3-token--sm" />
            <h2 id="renameTitle" className="m3-panel__title">
              {t('files_rename')}
            </h2>
          </div>
          <label className="flex flex-col gap-2 text-sm">
            <span className="text-on-surface-variant">{t('files_new_name')}</span>
            <input
              className="m3-field"
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
              {t('files_cancel')}
            </button>
            <button className={primaryButtonClass} type="button" onClick={() => void confirmRename()}>
              {t('files_rename')}
            </button>
          </div>
        </div>
      </div>

      <div
        className={cn(
          'm3-scrim z-[1200] flex items-center justify-center p-5 transition-opacity',
          editorOpen ? 'pointer-events-auto opacity-100' : 'pointer-events-none invisible opacity-0'
        )}
        aria-hidden={!editorOpen}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            setEditorOpen(false);
          }
        }}
      >
        <div
          className="m3-dialog m3-dialog--enter relative max-w-[780px]"
          role="dialog"
          aria-modal="true"
          aria-labelledby="editorTitle"
        >
          <button
            className="m3-icon-btn m3-state m3-icon-btn--sm absolute right-4 top-4"
            onClick={() => setEditorOpen(false)}
            aria-label={t('files_close')}
          >
            <X className="h-5 w-5" />
          </button>
          <div className="flex items-center gap-3 text-primary">
            <FileText className="m3-token m3-token--sm" />
            <h2 id="editorTitle" className="m3-panel__title">
              Edit: {basename(editorPath)}
            </h2>
          </div>
          {editorLoading ? (
            <div className="flex items-center gap-2 m3-body-medium m3-on-variant">
              <RefreshCw className="h-4 w-4 animate-spin" />
              {t('files_loading_contents')}
            </div>
          ) : editorError ? (
            <p className="text-sm text-red-500">{editorError}</p>
          ) : editorBinary ? (
            <p className="m3-body-medium m3-on-variant">
              {t('files_binary_notice')}
            </p>
          ) : (
            <>
              {editorTruncated && (
                <p className="m3-body-small m3-on-variant">
                  Showing the first chunk of the file. Download for full editing.
                </p>
              )}
              <textarea
                className="m3-field m3-field--mono min-h-[320px]"
                value={editorContent}
                onChange={(event) => setEditorContent(event.target.value)}
              />
            </>
          )}
          <div className="flex flex-wrap justify-end gap-3">
            <button className={toolbarButtonClass} type="button" onClick={() => setEditorOpen(false)}>
              {t('files_close')}
            </button>
            <button
              className={primaryButtonClass}
              type="button"
              onClick={() => void handleEditorSave()}
              disabled={editorLoading || editorBinary || filesBusy}
            >
              {t('files_save')}
            </button>
          </div>
        </div>
      </div>

      <div
        className={cn(
          'm3-scrim z-[1200] flex items-center justify-center p-5 transition-opacity',
          selectedScreenshot ? 'pointer-events-auto opacity-100' : 'pointer-events-none invisible opacity-0'
        )}
        aria-hidden={!selectedScreenshot}
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            closeScreenshot();
          }
        }}
      >
        <div
          className="m3-dialog m3-dialog--enter relative max-w-[980px]"
          role="dialog"
          aria-modal="true"
          aria-labelledby="screenshotTitle"
        >
          <button
            className="m3-icon-btn m3-state m3-icon-btn--sm absolute right-4 top-4"
            onClick={closeScreenshot}
            aria-label="Close"
          >
            <X className="h-5 w-5" />
          </button>
          <div className="flex items-center gap-3 text-primary">
            <Monitor className="m3-token m3-token--sm" />
            <h2 id="screenshotTitle" className="m3-panel__title">
              {screenshotPreviewTitle}
            </h2>
          </div>
          {selectedScreenshot && (
            <>
              <div className="m3-tray overflow-hidden p-2">
                <img
                  src={screenshotSrc(selectedScreenshot)}
                  alt={selectedScreenshot.caption || selectedScreenshot.filename}
                  className="max-h-[60vh] w-full object-contain"
                  decoding="async"
                />
              </div>
              <label className="flex flex-col gap-2 text-sm">
                <span className="text-on-surface-variant">{captionLabel}</span>
                <input
                  className="m3-field"
                  type="text"
                  value={captionDraft}
                  onChange={(event) => setCaptionDraft(event.target.value)}
                  placeholder={captionPlaceholder}
                />
              </label>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <span className="m3-body-small m3-on-variant">
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
