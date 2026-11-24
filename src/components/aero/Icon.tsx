import picture from '../../assets/icons/picture.png';
import folder from '../../assets/icons/folder.png';
import openFolder from '../../assets/icons/open.png';
import up from '../../assets/icons/up.png';
import back from '../../assets/icons/back.png';
import forward from '../../assets/icons/forward.png';
import refresh from '../../assets/icons/refresh.png';
import remove from '../../assets/icons/remove.png';
import search from '../../assets/icons/search.png';
import appearance from '../../assets/icons/appearance.png';

const icons = { picture, folder, openFolder, up, back, forward, refresh, remove, search, appearance };
export type IconName = keyof typeof icons;

export function AeroIcon({ name, size = 20 }: { name: IconName; size?: number }) {
  return <img src={icons[name]} width={size} height={size} alt="" aria-hidden="true" draggable={false} />;
}
