export const getPageForSection = (sectionId: string) => {
  if (sectionId === 'service-menu') return 'service-menu';
  if (sectionId === 'file-manager' || sectionId === 'gallery') return 'files';
  if (sectionId === 'macros' || sectionId === 'scripts') return 'automation';
  return 'home';
};
