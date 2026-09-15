import {
  BarChart3Icon,
  Building,
  ChevronDown,
  CodeIcon,
  FileIcon,
  HammerIcon,
  HomeIcon,
  LogInIcon,
  LogOutIcon,
  ShieldIcon,
  SkullIcon,
  TriangleAlertIcon,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { Link, LinkProps, useLocation } from 'react-router';
import { useQuery } from '@tanstack/react-query';
import { BASE_URL } from '~/api/index.ts';
import apiGetTypes from '~/api/types.ts';
import apiPostUserLogout from '~/api/user/logout.ts';
import { Button } from '~/components/ui/button.tsx';
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '~/components/ui/collapsible.tsx';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '~/components/ui/dropdown-menu.tsx';
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  useSidebar,
} from '~/components/ui/sidebar.tsx';
import { Skeleton } from '~/components/ui/skeleton.tsx';
import { ResponsiveTooltip, Tooltip, TooltipContent, TooltipTrigger } from '~/components/ui/tooltip.tsx';
import { useAuth } from '~/hooks/use-auth.tsx';
import { useToast } from '~/hooks/use-toast.ts';

const AutoCloseLink = ({ to, children, ...props }: LinkProps) => {
  const { isMobile, setOpenMobile } = useSidebar();

  const handleClick = () => {
    if (isMobile) {
      setOpenMobile(false);
    }
  };

  return (
    <Link to={to} onClick={handleClick} {...props}>
      {children}
    </Link>
  );
};

export function AppSidebar() {
  const { toast } = useToast();

  const location = useLocation(),
    [user, mutateUser, isUserLoading] = useAuth();

  const { data: types } = useQuery({ queryKey: ['types'], queryFn: () => apiGetTypes() });

  const [version, setVersion] = useState('');
  useEffect(() => {
    setVersion((window as unknown as { mcjars_version?: string }).mcjars_version ?? '');
  }, []);

  return (
    <>
      <Sidebar>
        <SidebarHeader>
          <AutoCloseLink to={'/'} className={'flex flex-row h-full w-fit items-center'}>
            <img src={'https://s3.mcjars.app/icons/vanilla.png'} alt={'MCJars'} className={'h-12 w-12'} />

            <div className={'flex flex-col ml-2'}>
              <h1 className={'text-xl font-semibold'}>MCJars</h1>
              <p className={'text-sm text-muted-foreground'}>{version}</p>
            </div>
          </AutoCloseLink>
        </SidebarHeader>
        <SidebarContent>
          <Collapsible defaultOpen className={'group/collapsible-information'}>
            <SidebarGroup>
              <SidebarGroupLabel asChild>
                <CollapsibleTrigger>
                  Information
                  <ChevronDown
                    className={
                      'ml-auto transition-transform group-data-[state=open]/collapsible-information:rotate-180'
                    }
                  />
                </CollapsibleTrigger>
              </SidebarGroupLabel>
              <CollapsibleContent>
                <SidebarGroupContent>
                  <SidebarMenu>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild isActive={location.pathname === '/'}>
                        <AutoCloseLink to={'/'}>
                          <HomeIcon className={'mr-2'} />
                          Home
                        </AutoCloseLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild isActive={location.pathname === '/lookup'}>
                        <AutoCloseLink to={'/lookup'}>
                          <FileIcon className={'mr-2'} />
                          File Lookup
                        </AutoCloseLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild isActive={location.pathname === '/configs'}>
                        <AutoCloseLink to={'/configs'}>
                          <FileIcon className={'mr-2'} />
                          Configs
                        </AutoCloseLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild isActive={location.pathname === '/job-status'}>
                        <AutoCloseLink to={'/job-status'}>
                          <HammerIcon className={'mr-2'} />
                          Job Status
                        </AutoCloseLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild>
                        <a href={'https://status.mcjars.app'} target={'_blank'} rel={'noreferrer'}>
                          <BarChart3Icon className={'mr-2'} />
                          Status Page
                        </a>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild>
                        <a href={'/api'} target={'_blank'} rel={'noreferrer'}>
                          <CodeIcon className={'mr-2'} />
                          API Documentation
                        </a>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                    <SidebarMenuItem>
                      <SidebarMenuButton asChild>
                        <a href={'https://github.com/mcjars/mcvcli'} target={'_blank'} rel={'noreferrer'}>
                          <CodeIcon className={'mr-2'} />
                          CLI Application
                        </a>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  </SidebarMenu>
                </SidebarGroupContent>
              </CollapsibleContent>
            </SidebarGroup>
          </Collapsible>

          {Object.entries(types ?? {}).map(([category, types]) => (
            <Collapsible defaultOpen className={'group/collapsible-types'}>
              <SidebarGroup>
                <SidebarGroupLabel asChild>
                  <CollapsibleTrigger className={'flex flex-row items-center justify-between'}>
                    {category[0].toUpperCase().concat(category.slice(1))}

                    <span className={'flex flex-row items-center'}>
                      ({types.reduce((acc, type) => acc + type.builds, 0)})
                      <ChevronDown
                        size={16}
                        className={'ml-1 transition-transform group-data-[state=open]/collapsible-types:rotate-180'}
                      />
                    </span>
                  </CollapsibleTrigger>
                </SidebarGroupLabel>
                <CollapsibleContent>
                  <SidebarGroupContent>
                    <SidebarMenu>
                      {!types ? (
                        <>
                          <Skeleton className={'mt-2'} />
                        </>
                      ) : (
                        <>
                          {types.map((type) => (
                            <SidebarMenuItem key={type.identifier}>
                              <Collapsible
                                defaultOpen={
                                  location.pathname.startsWith(`/${type.identifier}/`) || type.identifier === 'VANILLA'
                                }
                                className={'group/collapsible-type'}
                              >
                                <SidebarMenuButton
                                  asChild
                                  isActive={location.pathname.startsWith(`/${type.identifier}/`)}
                                >
                                  <CollapsibleTrigger className={'flex flex-row items-center'}>
                                    <img src={type.icon} alt={type.name} className={'h-6 w-6 rounded-md'} />
                                    {type.name}

                                    {type.experimental && (
                                      <ResponsiveTooltip>
                                        <Tooltip>
                                          <TooltipTrigger asChild>
                                            <TriangleAlertIcon size={16} className={'text-yellow-500'} />
                                          </TooltipTrigger>
                                          <TooltipContent className={'pointer-events-none'}>
                                            Experimental
                                          </TooltipContent>
                                        </Tooltip>
                                      </ResponsiveTooltip>
                                    )}
                                    {type.deprecated && (
                                      <ResponsiveTooltip>
                                        <Tooltip>
                                          <TooltipTrigger asChild>
                                            <SkullIcon size={16} className={'text-red-500'} />
                                          </TooltipTrigger>
                                          <TooltipContent className={'pointer-events-none'}>Deprecated</TooltipContent>
                                        </Tooltip>
                                      </ResponsiveTooltip>
                                    )}

                                    <div className={'ml-auto flex flex-row items-center'}>
                                      <p className={'mr-1'}>({type.builds})</p>

                                      <ChevronDown
                                        size={16}
                                        className={
                                          'transition-transform group-data-[state=open]/collapsible-type:rotate-180'
                                        }
                                      />
                                    </div>
                                  </CollapsibleTrigger>
                                </SidebarMenuButton>
                                <CollapsibleContent>
                                  <SidebarMenuSub>
                                    <SidebarMenuSubItem>
                                      <SidebarMenuSubButton
                                        asChild
                                        isActive={location.pathname === `/${type.identifier}/versions`}
                                      >
                                        <AutoCloseLink to={`/${type.identifier}/versions`}>Versions</AutoCloseLink>
                                      </SidebarMenuSubButton>
                                      <SidebarMenuSubButton
                                        asChild
                                        isActive={location.pathname === `/${type.identifier}/config`}
                                      >
                                        <AutoCloseLink to={`/${type.identifier}/config`}>Configs</AutoCloseLink>
                                      </SidebarMenuSubButton>
                                      <SidebarMenuSubButton
                                        asChild
                                        isActive={location.pathname === `/${type.identifier}/statistics`}
                                      >
                                        <AutoCloseLink to={`/${type.identifier}/statistics`}>Statistics</AutoCloseLink>
                                      </SidebarMenuSubButton>
                                    </SidebarMenuSubItem>
                                  </SidebarMenuSub>
                                </CollapsibleContent>
                              </Collapsible>
                            </SidebarMenuItem>
                          ))}
                        </>
                      )}
                    </SidebarMenu>
                  </SidebarGroupContent>
                </CollapsibleContent>
              </SidebarGroup>
            </Collapsible>
          ))}
        </SidebarContent>
        <SidebarFooter>
          {isUserLoading ? (
            <Button className={'w-full flex-row items-center justify-between'} variant={'secondary'}>
              <Skeleton className={'h-6 w-6 rounded-full'} />
              <Skeleton className={'h-6 w-20'} />
            </Button>
          ) : user ? (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button className={'w-full flex-row items-center justify-between'} variant={'secondary'}>
                  <img src={user.avatar} alt={user.name ?? `@${user.login}`} className={'h-6 w-6 rounded-full'} />
                  <span className={'ml-2 truncate'}>{user.name ?? `@${user.login}`}</span>
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align={'end'}>
                <DropdownMenuItem asChild>
                  <AutoCloseLink to={'/organizations'} className={'w-full'}>
                    <Building size={24} />
                    Organizations
                  </AutoCloseLink>
                </DropdownMenuItem>
                {user.admin && (
                  <DropdownMenuItem asChild>
                    <AutoCloseLink to={'/admin'} className={'w-full'}>
                      <ShieldIcon size={24} />
                      Administration
                    </AutoCloseLink>
                  </DropdownMenuItem>
                )}
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  onClick={() => {
                    toast({
                      title: 'Logging out...',
                      description: 'You are being logged out of MCJars.',
                    });

                    apiPostUserLogout().then(() => {
                      toast({
                        title: 'Logged out',
                        description: 'You have been logged out of MCJars.',
                      });

                      mutateUser(null, false);
                    });
                  }}
                >
                  <LogOutIcon size={24} />
                  Log Out
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          ) : (
            <a
              href={`${BASE_URL}/api/github`}
              className={'w-full'}
              onClick={() => {
                toast({
                  title: 'Logging in...',
                  description: 'You are being redirected to GitHub to login to MCJars.',
                });
              }}
            >
              <Button className={'w-full flex-row items-center justify-between'} variant={'secondary'}>
                <LogInIcon size={24} />
                <span className={'ml-2'}>Login</span>
              </Button>
            </a>
          )}
        </SidebarFooter>
      </Sidebar>
    </>
  );
}
