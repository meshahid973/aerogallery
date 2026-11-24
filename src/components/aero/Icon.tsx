import picture from '../../assets/icons/picture.png';
import folder from '../../assets/icons/folder.png';
import openFolder from '../../assets/icons/open.png';

const icons = { picture, folder, openFolder };
export type IconName = keyof typeof icons;

export function AeroIcon({ name, size = 20 }: { name: IconName; size?: number }) {
  return <img src={icons[name]} width={size} height={size} alt="" aria-hidden="true" draggable={false} />;
}
